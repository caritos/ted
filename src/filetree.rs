//! A read-only directory tree: state and key handling, no terminal code.
//! Paths are root-relative with `/` separators; the root itself is `""`.
//! The tree never creates, renames or deletes anything.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::screen::{Row, Screen, Span, Style};
use crate::text::{clip, pad, window_start};

/// Rows moved by PageUp / PageDown.
const PAGE: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum TreeAction {
    None,
    Back,
    ToggleGit,
    /// A root-relative path (with `/` separators) of a file to open.
    Open(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
}

/// One row of the flattened tree, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub expanded: bool,
    /// The directory could not be read. It has no child rows; the renderer
    /// shows a single `(cannot read)` row under it.
    pub unreadable: bool,
}

#[derive(Debug)]
struct Entry {
    name: String,
    kind: NodeKind,
}

/// The result of reading one directory: its entries, or why it failed.
#[derive(Debug)]
struct Listing {
    entries: Vec<Entry>,
    error: Option<String>,
}

pub struct FileTree {
    root: PathBuf,
    /// The root's display name, worked out once (see `root_name`).
    root_name: String,
    /// Paths of expanded directories (the root is always expanded).
    expanded: HashSet<String>,
    /// A listing for the root and every expanded directory.
    listings: HashMap<String, Listing>,
    /// Selection by path, never by row index.
    selected: Option<String>,
    message: String,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut tree = FileTree {
            root_name: root_name(&root),
            root,
            expanded: HashSet::new(),
            listings: HashMap::new(),
            selected: None,
            message: String::new(),
        };
        tree.refresh();
        tree
    }

    /// Re-read the root and every expanded directory. The selection stays on
    /// its path, or moves to the nearest ancestor that still exists.
    /// Directories that disappeared are no longer expanded.
    ///
    /// This reads synchronously, so a hung mount (e.g. a stale network
    /// share) can still block a refresh; accepted, as for the git view.
    pub fn refresh(&mut self) {
        let old = std::mem::take(&mut self.listings);
        let mut expanded = HashSet::new();
        let mut errors = Vec::new();
        self.reload("", &old, &mut expanded, &mut errors);
        self.expanded = expanded;
        // Never replace a message already showing (e.g. a refusal).
        if let Some(e) = errors.pop()
            && self.message.is_empty()
        {
            self.message = e;
        }
        self.ensure_selection();
    }

    /// Read `path`, then recurse into its subdirectories that were expanded.
    /// A message is only reported for a directory that was readable before.
    fn reload(
        &mut self,
        path: &str,
        old: &HashMap<String, Listing>,
        expanded: &mut HashSet<String>,
        errors: &mut Vec<String>,
    ) {
        let listing = self.read(path);
        if let Some(e) = &listing.error
            && old.get(path).is_none_or(|l| l.error.is_none())
        {
            errors.push(self.error_message(path, e));
        }
        expanded.insert(path.to_string());
        let children: Vec<String> = listing
            .entries
            .iter()
            .filter(|e| e.kind == NodeKind::Dir)
            .map(|e| join(path, &e.name))
            .filter(|p| self.expanded.contains(p))
            .collect();
        self.listings.insert(path.to_string(), listing);
        for child in children {
            self.reload(&child, old, expanded, errors);
        }
    }

    fn read(&self, path: &str) -> Listing {
        match self.read_entries(path) {
            Ok(entries) => Listing { entries, error: None },
            Err(e) => Listing { entries: Vec::new(), error: Some(e.to_string()) },
        }
    }

    fn read_entries(&self, path: &str) -> std::io::Result<Vec<Entry>> {
        let dir = if path.is_empty() { self.root.clone() } else { self.root.join(path) };
        let mut entries = Vec::new();
        for item in std::fs::read_dir(&dir)? {
            let item = item?;
            if item.file_name() == ".git" {
                continue;
            }
            // `file_type` costs no extra syscall on most platforms. Only a
            // symlink is followed (with `metadata`) to tell dir from file;
            // a broken link is just a file.
            let is_dir = match item.file_type() {
                Ok(t) if t.is_symlink() => {
                    std::fs::metadata(item.path()).is_ok_and(|m| m.is_dir())
                }
                Ok(t) => t.is_dir(),
                Err(_) => false,
            };
            let kind = if is_dir { NodeKind::Dir } else { NodeKind::File };
            entries.push(Entry { name: item.file_name().to_string_lossy().into_owned(), kind });
        }
        // Directories first, then a trimmed case-insensitive key, then the
        // exact name. The key is computed once per entry, not per comparison.
        entries.sort_by_cached_key(|e| {
            (e.kind == NodeKind::File, e.name.trim().to_lowercase(), e.name.clone())
        });
        Ok(entries)
    }

    fn display_name(&self, path: &str) -> String {
        if path.is_empty() {
            return self.root_name.clone();
        }
        path.rsplit('/').next().unwrap_or(path).to_string()
    }

    fn error_message(&self, path: &str, error: &str) -> String {
        format!("Cannot read {}: {}", self.display_name(path), error)
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// The flattened tree. If the root itself cannot be read this is a single
    /// unreadable root row and nothing is selectable.
    pub(crate) fn visible(&self) -> Vec<VisibleRow> {
        let mut rows = Vec::new();
        if self.listings.get("").is_some_and(|l| l.error.is_some()) {
            rows.push(VisibleRow {
                depth: 0,
                name: self.display_name(""),
                path: String::new(),
                kind: NodeKind::Dir,
                expanded: true,
                unreadable: true,
            });
        } else {
            self.walk("", 0, &mut rows);
        }
        rows
    }

    fn walk(&self, path: &str, depth: usize, rows: &mut Vec<VisibleRow>) {
        let Some(listing) = self.listings.get(path) else { return };
        for entry in &listing.entries {
            let child = join(path, &entry.name);
            let expanded = entry.kind == NodeKind::Dir && self.expanded.contains(&child);
            let unreadable =
                expanded && self.listings.get(&child).is_some_and(|l| l.error.is_some());
            rows.push(VisibleRow {
                depth,
                name: entry.name.clone(),
                path: child.clone(),
                kind: entry.kind,
                expanded,
                unreadable,
            });
            if expanded {
                self.walk(&child, depth + 1, rows);
            }
        }
    }

    /// Keep `selected` on a visible row: else its nearest visible ancestor,
    /// else the first row; `None` when there are no rows to select.
    fn ensure_selection(&mut self) {
        let rows = self.visible();
        if rows.first().is_some_and(|r| r.path.is_empty()) {
            self.selected = None; // unreadable root marker
            return;
        }
        let mut candidate = self.selected.clone();
        while let Some(path) = candidate {
            if rows.iter().any(|r| r.path == path) {
                self.selected = Some(path);
                return;
            }
            candidate = path.rsplit_once('/').map(|(parent, _)| parent.to_string());
        }
        self.selected = rows.first().map(|r| r.path.clone());
    }

    fn expand(&mut self, path: &str) {
        let listing = self.read(path);
        if let Some(e) = &listing.error {
            self.message = self.error_message(path, e);
        }
        self.listings.insert(path.to_string(), listing);
        self.expanded.insert(path.to_string());
    }

    /// Collapse `path` and everything under it; a selection inside moves to
    /// the directory itself.
    fn collapse(&mut self, path: &str) {
        let prefix = format!("{path}/");
        self.expanded.retain(|p| p != path && !p.starts_with(&prefix));
        self.listings.retain(|p, _| p != path && !p.starts_with(&prefix));
        if self.selected.as_deref().is_some_and(|s| s.starts_with(&prefix)) {
            self.selected = Some(path.to_string());
        }
    }

    /// Draw the tree into `height` rows: a body of `height - 2` rows, then
    /// the status row and the message row.
    pub fn render(&self, width: usize, height: usize) -> Screen {
        let body_h = height.saturating_sub(2);
        let visible = self.visible();
        let selected = self.selected_row(&visible);

        // Display lines (text and style), and the line holding the selection.
        let mut lines: Vec<(String, Style)> = Vec::new();
        let mut selected_line = 0;
        for (i, row) in visible.iter().enumerate() {
            let indent = "  ".repeat(row.depth);
            let text = match row.kind {
                NodeKind::Dir => {
                    let marker = if row.expanded { "v " } else { "> " };
                    format!("{indent}{marker}{}/", row.name)
                }
                NodeKind::File => format!("{indent}  {}", row.name),
            };
            let style = if selected == Some(i) {
                selected_line = lines.len();
                Style::Selected
            } else if row.kind == NodeKind::Dir {
                Style::Dir
            } else {
                Style::Normal
            };
            lines.push((text, style));
            if row.unreadable {
                lines.push((format!("{indent}    (cannot read)"), Style::Dim));
            }
        }

        let first = window_start(selected_line, body_h);
        let mut rows: Vec<Row> = lines
            .into_iter()
            .skip(first)
            .take(body_h)
            .map(|(text, style)| {
                // Padded so the selection bar spans the row.
                let text =
                    if style == Style::Selected { pad(&text, width) } else { clip(&text, width) };
                vec![Span { text, style }]
            })
            .collect();
        rows.resize_with(body_h, Vec::new);

        let (status, message) = if height >= 2 {
            (pad(&self.status_text(&visible), width), clip(&self.message, width))
        } else {
            (String::new(), String::new())
        };
        Screen { rows, status, message, cursor: None }
    }

    fn status_text(&self, visible: &[VisibleRow]) -> String {
        let name = self.display_name("");
        if self.listings.get("").is_some_and(|l| l.error.is_some()) {
            format!("Files: {name}  Error")
        } else if visible.is_empty() {
            format!("Files: {name}  (empty)")
        } else {
            format!("Files: {name}  {} entries visible", visible.len())
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TreeAction {
        self.message.clear();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('g') if ctrl => return TreeAction::ToggleGit,
            KeyCode::Char('t') if ctrl => return TreeAction::Back,
            KeyCode::Char('q') if plain => return TreeAction::Back,
            KeyCode::Esc => return TreeAction::Back,
            _ if !plain => {}
            KeyCode::Up => self.move_by(-1),
            KeyCode::Down => self.move_by(1),
            KeyCode::PageUp => self.move_by(-(PAGE as isize)),
            KeyCode::PageDown => self.move_by(PAGE as isize),
            KeyCode::Home => self.move_by(isize::MIN / 2),
            KeyCode::End => self.move_by(isize::MAX / 2),
            KeyCode::Right => self.right(),
            KeyCode::Left => self.left(),
            KeyCode::Enter => return self.enter(),
            _ => {}
        }
        TreeAction::None
    }

    /// The selected row and its index in `rows`.
    fn selected_row(&self, rows: &[VisibleRow]) -> Option<usize> {
        let sel = self.selected.as_deref()?;
        rows.iter().position(|r| r.path == sel)
    }

    fn move_by(&mut self, delta: isize) {
        let rows = self.visible();
        if rows.is_empty() || self.selected.is_none() {
            return;
        }
        let current = self.selected_row(&rows).unwrap_or(0) as isize;
        let target = current.saturating_add(delta).clamp(0, rows.len() as isize - 1);
        self.selected = Some(rows[target as usize].path.clone());
    }

    fn right(&mut self) {
        let rows = self.visible();
        let Some(i) = self.selected_row(&rows) else { return };
        let row = &rows[i];
        if row.kind != NodeKind::Dir {
            return;
        }
        if !row.expanded {
            let path = row.path.clone();
            self.expand(&path);
        } else if let Some(next) = rows.get(i + 1).filter(|n| n.depth > row.depth) {
            self.selected = Some(next.path.clone());
        }
    }

    fn left(&mut self) {
        let rows = self.visible();
        let Some(i) = self.selected_row(&rows) else { return };
        let row = &rows[i];
        if row.kind == NodeKind::Dir && row.expanded {
            let path = row.path.clone();
            self.collapse(&path);
        } else if let Some((parent, _)) = row.path.rsplit_once('/') {
            self.selected = Some(parent.to_string());
        }
    }

    fn enter(&mut self) -> TreeAction {
        let rows = self.visible();
        let Some(i) = self.selected_row(&rows) else { return TreeAction::None };
        let row = &rows[i];
        match (row.kind, row.expanded) {
            (NodeKind::File, _) => return TreeAction::Open(row.path.clone()),
            (NodeKind::Dir, true) => {
                let path = row.path.clone();
                self.collapse(&path);
            }
            (NodeKind::Dir, false) => {
                let path = row.path.clone();
                self.expand(&path);
            }
        }
        TreeAction::None
    }
}

/// The last component of `root`. A root such as `.` or `sub/..` has none,
/// so the real directory's name is used (`canonicalize` resolves it against
/// the current directory); failing that, the path as given (e.g. `/`).
/// Rendering sanitizes it like any other name.
fn root_name(root: &Path) -> String {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    name(root)
        .or_else(|| root.canonicalize().ok().and_then(|p| name(&p)))
        .unwrap_or_else(|| root.display().to_string())
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

#[cfg(test)]
mod tests;
