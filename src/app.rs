//! The editor plus the git view and the file tree, and which one is showing.

use std::path::PathBuf;
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;
use crate::editor::{Editor, Flow, View};
use crate::filetree::{FileTree, TreeAction};
use crate::git::Git;
use crate::gitview::{GitAction, GitView};
use crate::screen::{Screen, Span, Style};

/// Largest file the views will open (64 MiB); `Buffer::open` reads it all.
const MAX_OPEN_BYTES: u64 = 64 * 1024 * 1024;

/// Which view is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Editor,
    GitView,
    Files,
}

pub struct App {
    editor: Editor,
    /// Directory the git view and the file tree look at; opened paths are
    /// relative to it.
    location: PathBuf,
    git: Box<dyn Git>,
    /// Created on the first Ctrl-G and kept, so the baseline survives.
    git_view: Option<GitView>,
    /// Created on the first Ctrl-T and kept, so expansion and selection
    /// survive between visits.
    file_tree: Option<FileTree>,
    mode: Mode,
}

impl App {
    pub fn new(editor: Editor, location: PathBuf, git: Box<dyn Git>) -> Self {
        App { editor, location, git, git_view: None, file_tree: None, mode: Mode::Editor }
    }

    pub fn in_git_view(&self) -> bool {
        self.mode == Mode::GitView
    }

    pub fn in_file_tree(&self) -> bool {
        self.mode == Mode::Files
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: SystemTime) -> Flow {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.mode == Mode::Editor {
            if ctrl && key.code == KeyCode::Char('g') {
                self.enter_git_view(now);
                return Flow::Continue;
            }
            if ctrl && key.code == KeyCode::Char('t') {
                self.enter_file_tree();
                return Flow::Continue;
            }
            return self.editor.handle_key(key);
        }

        // Ctrl-Q follows the editor's quit rules; its warning shows here.
        if ctrl && key.code == KeyCode::Char('q') {
            let flow = self.editor.handle_key(key);
            self.set_view_message(self.editor.message().to_string());
            return flow;
        }
        // Any other key cancels a pending quit warning, as in the editor.
        self.editor.cancel_quit();

        match self.mode {
            Mode::Editor => {}
            Mode::GitView => {
                let Some(view) = &mut self.git_view else {
                    self.mode = Mode::Editor;
                    return Flow::Continue;
                };
                match view.handle_key(key, self.git.as_ref(), now) {
                    GitAction::None => {}
                    GitAction::Back => self.mode = Mode::Editor,
                    GitAction::ToggleTree => self.enter_file_tree(),
                    GitAction::Open(rel) => self.open(&rel),
                }
            }
            Mode::Files => {
                let Some(tree) = &mut self.file_tree else {
                    self.mode = Mode::Editor;
                    return Flow::Continue;
                };
                match tree.handle_key(key) {
                    TreeAction::None => {}
                    TreeAction::Back => self.mode = Mode::Editor,
                    TreeAction::ToggleGit => self.enter_git_view(now),
                    TreeAction::Open(rel) => self.open(&rel),
                }
            }
        }
        Flow::Continue
    }

    /// Refreshes the view that is showing; does nothing in the editor.
    pub fn tick(&mut self, now: SystemTime) {
        match self.mode {
            Mode::Editor => {}
            Mode::GitView => {
                if let Some(view) = &mut self.git_view {
                    view.refresh(self.git.as_ref(), now);
                }
            }
            Mode::Files => {
                if let Some(tree) = &mut self.file_tree {
                    tree.refresh();
                }
            }
        }
    }

    pub fn screen(&mut self, width: usize, height: usize, now: SystemTime) -> Screen {
        match (self.mode, &self.git_view, &self.file_tree) {
            (Mode::GitView, Some(view), _) => view.render(width, height, now),
            (Mode::Files, _, Some(tree)) => tree.render(width, height),
            _ => editor_screen(self.editor.view(width, height)),
        }
    }

    fn enter_git_view(&mut self, now: SystemTime) {
        // Ctrl-G is a key like any other: it cancels a pending quit warning.
        self.editor.cancel_quit();
        let view = self.git_view.get_or_insert_with(GitView::new);
        view.refresh(self.git.as_ref(), now);
        self.mode = Mode::GitView;
    }

    fn enter_file_tree(&mut self) {
        // Ctrl-T is a key like any other: it cancels a pending quit warning.
        self.editor.cancel_quit();
        match &mut self.file_tree {
            Some(tree) => tree.refresh(),
            // `new` lists the root, so a fresh tree needs no extra refresh.
            None => self.file_tree = Some(FileTree::new(self.location.clone())),
        }
        self.mode = Mode::Files;
    }

    /// Shows `message` in the view that is showing (not in the editor).
    fn set_view_message(&mut self, message: String) {
        match self.mode {
            Mode::Editor => {}
            Mode::GitView => {
                if let Some(view) = &mut self.git_view {
                    view.set_message(message);
                }
            }
            Mode::Files => {
                if let Some(tree) = &mut self.file_tree {
                    tree.set_message(message);
                }
            }
        }
    }

    /// Opens `rel` (relative to `location`) in the editor and returns to it.
    /// A refusal is shown in the view that asked, which stays showing.
    fn open(&mut self, rel: &str) {
        match self.load(rel) {
            Ok(buffer) => {
                self.editor.load(buffer);
                self.mode = Mode::Editor;
            }
            Err(message) => self.set_view_message(message),
        }
    }

    /// Reads `rel` from disk, unless that would discard unsaved edits.
    fn load(&self, rel: &str) -> Result<Buffer, String> {
        if self.editor.buffer().is_modified() {
            return Err("Unsaved changes: save or quit first".to_string());
        }
        // `Buffer::open` treats a missing file as new, and a later save
        // would recreate it. `metadata` follows symlinks, so a dangling
        // link or an unreadable parent is refused too.
        let path = self.location.join(rel);
        let Ok(meta) = std::fs::metadata(&path) else {
            return Err("File no longer exists".to_string());
        };
        // Reading a FIFO or a device can block forever (or never end).
        if !meta.is_file() {
            return Err("Not a regular file".to_string());
        }
        if meta.len() > MAX_OPEN_BYTES {
            return Err("File too large".to_string());
        }
        Buffer::open(&path).map_err(|e| format!("Open failed: {e}"))
    }
}

/// The editor's frame as a `Screen`: consecutive chars of one style share a
/// span, so a plain row is exactly one `Normal` span.
fn editor_screen(view: View) -> Screen {
    Screen {
        rows: view.lines.iter().zip(&view.styles).map(|(text, styles)| spans(text, styles)).collect(),
        status: view.status,
        message: view.message,
        cursor: Some(view.cursor),
    }
}

fn spans(text: &str, styles: &[Style]) -> Vec<Span> {
    let mut row: Vec<Span> = Vec::new();
    for (c, &style) in text.chars().zip(styles) {
        match row.last_mut() {
            Some(span) if span.style == style => span.text.push(c),
            _ => row.push(Span { text: c.to_string(), style }),
        }
    }
    if row.is_empty() {
        row.push(Span { text: String::new(), style: Style::Normal });
    }
    row
}

#[cfg(test)]
mod tests;
