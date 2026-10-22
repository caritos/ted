//! The editor plus the git view and the file tree, and which one is showing.

use std::path::{Path, PathBuf};
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
    /// Open buffers, never empty; `current` indexes the showing one.
    editors: Vec<Editor>,
    current: usize,
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
    /// Ctrl-W warned about a modified buffer; the next Ctrl-W closes it.
    close_armed: bool,
    /// Ctrl-Q warned about modified buffers; the next Ctrl-Q quits.
    quit_armed: bool,
}

impl App {
    pub fn new(editor: Editor, location: PathBuf, git: Box<dyn Git>) -> Self {
        App { editors: vec![editor], current: 0, location, git, git_view: None, file_tree: None, mode: Mode::Editor, close_armed: false, quit_armed: false }
    }

    pub fn in_git_view(&self) -> bool {
        self.mode == Mode::GitView
    }

    pub fn in_file_tree(&self) -> bool {
        self.mode == Mode::Files
    }

    pub fn editor(&self) -> &Editor {
        &self.editors[self.current]
    }

    fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editors[self.current]
    }

    pub fn buffer_count(&self) -> usize {
        self.editors.len()
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Switches to the next (`forward`) or previous buffer, wrapping, and
    /// returns to the editor.
    fn cycle(&mut self, forward: bool) {
        let n = self.editors.len();
        self.current = if forward { (self.current + 1) % n } else { (self.current + n - 1) % n };
        self.mode = Mode::Editor;
    }

    /// Closes the current buffer. A modified one needs a second Ctrl-W.
    /// The previous neighbour (or the first) becomes current; closing the
    /// last buffer leaves one empty unnamed buffer.
    fn close_current(&mut self) {
        if self.editor().buffer().is_modified() && !self.close_armed {
            self.close_armed = true;
            self.editor_mut()
                .set_message("Unsaved changes! Press Ctrl-W again to close without saving.".to_string());
            return;
        }
        self.close_armed = false;
        self.editors.remove(self.current);
        if self.editors.is_empty() {
            self.editors.push(Editor::new(Buffer::new()));
        }
        self.current = self.current.saturating_sub(1);
    }

    /// Quits at once when no buffer is modified; otherwise warns, and a
    /// second Ctrl-Q (with no other key between) quits.
    fn quit(&mut self) -> Flow {
        let modified = self.editors.iter().filter(|e| e.buffer().is_modified()).count();
        if modified == 0 || self.quit_armed {
            return Flow::Quit;
        }
        self.quit_armed = true;
        let noun = if modified == 1 { "buffer" } else { "buffers" };
        let message =
            format!("Unsaved changes in {modified} {noun}! Press Ctrl-Q again to quit without saving.");
        match self.mode {
            Mode::Editor => self.editor_mut().set_message(message),
            _ => self.set_view_message(message),
        }
        Flow::Continue
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: SystemTime) -> Flow {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let is_quit = ctrl && key.code == KeyCode::Char('q');
        // Any key but Ctrl-Q withdraws a pending quit warning.
        if !is_quit && std::mem::take(&mut self.quit_armed) {
            // The warning sits in the editor when armed there, in the view otherwise.
            self.editor_mut().set_message(String::new());
            self.set_view_message(String::new());
        }
        if is_quit {
            // Ctrl-Q is another key to a pending close warning; its own
            // warning replaces that message.
            self.close_armed = false;
            return self.quit();
        }
        let is_close = ctrl && key.code == KeyCode::Char('w');
        // Any key but Ctrl-W withdraws a pending close warning.
        if !is_close && std::mem::take(&mut self.close_armed) {
            self.editor_mut().set_message(String::new());
        }
        if is_close {
            if self.mode == Mode::Editor {
                self.close_current();
            }
            return Flow::Continue;
        }
        if ctrl && matches!(key.code, KeyCode::Char('n') | KeyCode::Char('p')) {
            self.cycle(key.code == KeyCode::Char('n'));
            return Flow::Continue;
        }
        if self.mode == Mode::Editor {
            if ctrl && key.code == KeyCode::Char('g') {
                self.enter_git_view(now);
                return Flow::Continue;
            }
            if ctrl && key.code == KeyCode::Char('t') {
                self.enter_file_tree();
                return Flow::Continue;
            }
            return self.editor_mut().handle_key(key);
        }

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
        let n = self.editors.len();
        let tag = if n > 1 { format!("[{}/{}]", self.current + 1, n) } else { String::new() };
        self.editor_mut().set_tag(tag);
        match (self.mode, &self.git_view, &self.file_tree) {
            (Mode::GitView, Some(view), _) => view.render(width, height, now),
            (Mode::Files, _, Some(tree)) => tree.render(width, height),
            _ => editor_screen(self.editor_mut().view(width, height)),
        }
    }

    fn enter_git_view(&mut self, now: SystemTime) {
        let view = self.git_view.get_or_insert_with(GitView::new);
        view.refresh(self.git.as_ref(), now);
        self.mode = Mode::GitView;
    }

    fn enter_file_tree(&mut self) {
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
                self.add_or_switch(buffer);
                self.mode = Mode::Editor;
            }
            Err(message) => self.set_view_message(message),
        }
    }

    /// Makes `buffer` current: switches to the buffer already holding the
    /// same file, replaces a lone pristine unnamed buffer, or appends.
    fn add_or_switch(&mut self, buffer: Buffer) {
        if let Some(path) = buffer.path() {
            let found = self
                .editors
                .iter()
                .position(|e| e.buffer().path().is_some_and(|p| same_file(p, path)));
            if let Some(i) = found {
                self.current = i;
                return;
            }
        }
        let only = &self.editors[0];
        let pristine = self.editors.len() == 1
            && only.buffer().path().is_none()
            && !only.buffer().is_modified()
            && only.buffer().text().is_empty();
        if pristine {
            self.editors[0] = Editor::new(buffer);
            self.current = 0;
        } else {
            self.editors.push(Editor::new(buffer));
            self.current = self.editors.len() - 1;
        }
    }

    /// Reads `rel` from disk.
    fn load(&self, rel: &str) -> Result<Buffer, String> {
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

/// Whether two paths name the same file: canonical forms when both resolve,
/// else the paths as given.
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
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
    // A missing style falls back to Normal, so text can never be dropped.
    for (i, c) in text.chars().enumerate() {
        let style = styles.get(i).copied().unwrap_or(Style::Normal);
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
