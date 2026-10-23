//! "Open file by name": a query box over a list of project files, ranked by
//! `fuzzy`. State, key handling and rendering; no terminal code. The app
//! supplies the file list (from git, or `walk_files`).

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::fuzzy;
use crate::screen::{Row, Screen, Span, Style};
use crate::text::{clip, pad, window_start};

/// Most files listed; a bigger project is cut (and the status says so).
pub const MAX_FILES: usize = 100_000;

/// Directories `walk_files` never enters (besides hidden ones).
const SKIPPED_DIRS: [&str; 2] = ["target", "node_modules"];

/// Rows PageUp / PageDown move.
const PAGE: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum FinderAction {
    None,
    Back,
    /// Open this path (relative to the app's location) and return to the editor.
    Open(String),
}

pub struct Finder {
    files: Vec<String>,
    /// The list was cut at `MAX_FILES`.
    cut: bool,
    query: String,
    /// Indexes into `files`, best first.
    ranked: Vec<usize>,
    selected: usize,
    message: String,
}

impl Finder {
    pub fn new(files: Vec<String>, cut: bool) -> Self {
        let ranked = (0..files.len()).collect();
        Finder { files, cut, query: String::new(), ranked, selected: 0, message: String::new() }
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// The path under the selection, if the list is not empty.
    pub fn selected_path(&self) -> Option<&str> {
        self.ranked.get(self.selected).map(|&i| self.files[i].as_str())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FinderAction {
        self.message.clear();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let last = self.ranked.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => return FinderAction::Back,
            KeyCode::Char('e') if ctrl => return FinderAction::Back,
            KeyCode::Enter => {
                if let Some(path) = self.selected_path() {
                    return FinderAction::Open(path.to_string());
                }
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(PAGE),
            KeyCode::PageDown => self.selected = (self.selected + PAGE).min(last),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    self.requery();
                }
            }
            KeyCode::Char(c) if !ctrl && !alt => {
                self.query.push(c);
                self.requery();
            }
            _ => {}
        }
        FinderAction::None
    }

    /// Text arriving in one piece (a paste): line breaks never get in.
    pub fn paste(&mut self, text: &str) {
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        self.requery();
    }

    fn requery(&mut self) {
        self.ranked = fuzzy::rank(&self.query, &self.files);
        self.selected = 0;
    }

    /// A body of `height - 2` rows, then the status and the query row.
    pub fn render(&self, width: usize, height: usize) -> Screen {
        let body_h = height.saturating_sub(2);
        let selected = self.selected.min(self.ranked.len().saturating_sub(1));
        let first = window_start(selected, body_h);
        let mut rows: Vec<Row> = self
            .ranked
            .iter()
            .enumerate()
            .skip(first)
            .take(body_h)
            .map(|(n, &i)| {
                let text = format!("  {}", self.files[i]);
                if n == selected {
                    vec![Span { text: pad(&text, width), style: Style::Selected }]
                } else {
                    vec![Span { text: clip(&text, width), style: Style::Normal }]
                }
            })
            .collect();
        if self.ranked.is_empty() && body_h > 0 {
            let note = if self.files.is_empty() { "  No files" } else { "  No match" };
            rows.push(vec![Span { text: clip(note, width), style: Style::Dim }]);
        }
        rows.resize_with(body_h, Vec::new);
        let (status, message) = if height >= 2 {
            let cut = if self.cut { " (list cut)" } else { "" };
            let status = format!("Open file: {} of {} files{cut}", self.ranked.len(), self.files.len());
            let line = if self.message.is_empty() {
                // The end of a long query is where the typing happens.
                let line = format!("Open: {}_", self.query);
                let chars = line.chars().count();
                if chars > width && width > 0 {
                    format!("\u{2026}{}", line.chars().skip(chars - (width - 1)).collect::<String>())
                } else {
                    line
                }
            } else {
                self.message.clone()
            };
            (pad(&status, width), clip(&line, width))
        } else {
            (String::new(), String::new())
        };
        Screen { rows, status, message, cursor: None }
    }
}

/// Every file under `root` as a `/`-separated path relative to it, sorted:
/// hidden names, `target` and `node_modules` are skipped and directory
/// symlinks are not followed. At most `MAX_FILES`; the flag says it was cut.
pub fn walk_files(root: &Path) -> (Vec<String>, bool) {
    let mut files = Vec::new();
    let mut cut = false;
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let rel = format!("{prefix}{name}");
            if kind.is_dir() {
                if !SKIPPED_DIRS.contains(&name.as_str()) {
                    pending.push((entry.path(), format!("{rel}/")));
                }
            } else if kind.is_file() || (kind.is_symlink() && entry.path().is_file()) {
                if files.len() >= MAX_FILES {
                    cut = true;
                } else {
                    files.push(rel);
                }
            }
        }
    }
    files.sort();
    (files, cut)
}

#[cfg(test)]
mod tests;
