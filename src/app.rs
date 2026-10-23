//! The editor plus the git view and the file tree, and which one is showing.

use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;
use crate::bufferlist::{BufferList, ListAction};
use crate::clipboard::{Clipboard, SystemClipboard};
use crate::command::{self, Command};
use crate::editor::{Editor, Flow, View};
use crate::filetree::{FileTree, TreeAction};
use crate::git::Git;
use crate::gitview::{GitAction, GitView};
use crate::help::{Help, HelpAction};
use crate::screen::{Screen, Span, Style};

/// Largest file the views will open (64 MiB); `Buffer::open` reads it all.
const MAX_OPEN_BYTES: u64 = 64 * 1024 * 1024;

/// Which view is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Editor,
    GitView,
    Files,
    Buffers,
    Help,
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
    /// Created on the first Ctrl-O; reset to the current buffer on each visit.
    buffer_list: Option<BufferList>,
    mode: Mode,
    /// Ctrl-W warned about a modified buffer; the next Ctrl-W closes it.
    close_armed: bool,
    /// Ctrl-Q warned about modified buffers; the next Ctrl-Q quits.
    quit_armed: bool,
    help: Help,
    /// The mode F1 was pressed in; closing help returns there.
    help_from: Mode,
    /// The `F1: help` hint is still waiting for the first key.
    hint_pending: bool,
    /// Line number gutter, one setting for every buffer; applied to the
    /// current editor before each draw.
    line_numbers: bool,
    /// The last accepted search term, shared by every buffer.
    search: String,
    clipboard: Box<dyn Clipboard>,
    /// What the last copy or cut took; paste falls back to it when the
    /// system clipboard has no text.
    register: String,
}

impl App {
    /// An app that uses the system clipboard.
    pub fn new(editor: Editor, location: PathBuf, git: Box<dyn Git>) -> Self {
        Self::with_clipboard(editor, location, git, Box::new(SystemClipboard::new()))
    }

    pub fn with_clipboard(
        mut editor: Editor,
        location: PathBuf,
        git: Box<dyn Git>,
        clipboard: Box<dyn Clipboard>,
    ) -> Self {
        // One-time hint; the first key clears it like any message.
        editor.set_message("F1: help".to_string());
        App {
            clipboard,
            register: String::new(),
            editors: vec![editor],
            current: 0,
            location,
            git,
            git_view: None,
            file_tree: None,
            buffer_list: None,
            mode: Mode::Editor,
            close_armed: false,
            quit_armed: false,
            help: Help::new(),
            help_from: Mode::Editor,
            hint_pending: true,
            line_numbers: true,
            search: String::new(),
        }
    }

    pub fn in_help(&self) -> bool {
        self.mode == Mode::Help
    }

    fn enter_help(&mut self) {
        if self.mode == Mode::Help {
            return;
        }
        if self.mode == Mode::Editor {
            self.editor_mut().set_message(String::new());
        }
        // The commit picker stays open, so closing help returns to it.
        self.help_from = self.mode;
        self.help.reset();
        self.mode = Mode::Help;
    }

    pub fn in_git_view(&self) -> bool {
        self.mode == Mode::GitView
    }

    pub fn in_file_tree(&self) -> bool {
        self.mode == Mode::Files
    }

    pub fn in_buffer_list(&self) -> bool {
        self.mode == Mode::Buffers
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

    /// Makes buffer `i` the showing one; the selection of the buffer left and
    /// of the one entered is dropped.
    fn set_current(&mut self, i: usize) {
        self.editors[self.current].clear_selection();
        self.current = i;
        self.editors[i].clear_selection();
    }

    /// Switches to the next (`forward`) or previous buffer, wrapping, and
    /// returns to the editor.
    fn cycle(&mut self, forward: bool) {
        let n = self.editors.len();
        let next = if forward { (self.current + 1) % n } else { (self.current + n - 1) % n };
        self.set_current(next);
        self.close_picker();
        self.mode = Mode::Editor;
    }

    /// Closes the changes view's commit picker so nothing stale is waiting on return.
    fn close_picker(&mut self) {
        if let Some(view) = &mut self.git_view {
            view.close_picker();
        }
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
        self.editors[self.current].clear_selection();
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
        // The first key of any kind removes the startup hint, wherever it lands.
        if std::mem::take(&mut self.hint_pending) {
            for editor in &mut self.editors {
                if editor.message() == "F1: help" {
                    editor.set_message(String::new());
                }
            }
        }
        // Whatever buffer is current gets the shared term before it sees the key.
        if self.mode == Mode::Editor && !self.search.is_empty() {
            let term = self.search.clone();
            self.editor_mut().set_last_search(&term);
        }
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
            if self.mode == Mode::Editor {
                self.editor_mut().cancel_prompt();
            }
            return self.quit();
        }
        // An open prompt gets every other key, so Ctrl-G/T/O/N/P/W, F1 and `?`
        // are text or ignored there instead of their global action.
        if self.mode == Mode::Editor && self.editor().in_prompt() {
            self.close_armed = false;
            let flow = self.editor_mut().handle_key(key);
            return self.finish_key(flow, now);
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
        if ctrl && key.code == KeyCode::Char('o') && self.mode != Mode::Buffers {
            self.enter_buffer_list();
            return Flow::Continue;
        }
        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        // `?` is text in the editor, so only the views (no text input) take it.
        let help_key = key.code == KeyCode::F(1) || (plain && key.code == KeyCode::Char('?') && self.mode != Mode::Editor);
        if help_key && self.mode != Mode::Help {
            self.enter_help();
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
            if key.code == KeyCode::F(2) {
                self.line_numbers = !self.line_numbers;
                return Flow::Continue;
            }
            // Not in a prompt (it took the key above) and not in a view.
            if ctrl && matches!(key.code, KeyCode::Char('c' | 'k' | 'v')) {
                self.editor_mut().set_message(String::new());
                match key.code {
                    KeyCode::Char('c') => self.copy(),
                    KeyCode::Char('k') => self.cut(),
                    _ => self.paste_clipboard(),
                }
                return Flow::Continue;
            }
            let flow = self.editor_mut().handle_key(key);
            return self.finish_key(flow, now);
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
            Mode::Buffers => {
                let n = self.editors.len();
                let Some(list) = &mut self.buffer_list else {
                    self.mode = Mode::Editor;
                    return Flow::Continue;
                };
                match list.handle_key(key, n) {
                    ListAction::None => {}
                    ListAction::Back => self.mode = Mode::Editor,
                    ListAction::Switch(i) => {
                        self.set_current(i.min(n - 1));
                        self.mode = Mode::Editor;
                    }
                    ListAction::ToggleGit => self.enter_git_view(now),
                    ListAction::ToggleTree => self.enter_file_tree(),
                }
            }
            Mode::Help => match self.help.handle_key(key) {
                HelpAction::None => {}
                HelpAction::Back => self.mode = self.help_from,
                HelpAction::ToggleGit => self.enter_git_view(now),
                HelpAction::ToggleTree => {
                    self.close_picker();
                    self.enter_file_tree();
                }
            },
        }
        Flow::Continue
    }

    /// Remembers `text` as the last copy: in the register always, in the
    /// system clipboard when that works.
    fn store(&mut self, text: &str) {
        self.register.clear();
        self.register.push_str(text);
        let _ = self.clipboard.set(text);
    }

    /// Ctrl-C / `copy`: the selection goes to the clipboard; text and
    /// selection stay.
    fn copy(&mut self) {
        match self.editor().selected_text() {
            Some(text) => {
                self.store(&text);
                let message = format!("Copied {} chars", text.chars().count());
                self.editor_mut().set_message(message);
            }
            None => self.editor_mut().set_message("Nothing selected".to_string()),
        }
    }

    /// Ctrl-K / `cut`: like copy, then the selection is deleted (one undo step).
    fn cut(&mut self) {
        match self.editor_mut().cut_selection() {
            Some(text) => {
                self.store(&text);
                let message = format!("Cut {} chars", text.chars().count());
                self.editor_mut().set_message(message);
            }
            None => self.editor_mut().set_message("Nothing selected".to_string()),
        }
    }

    /// Ctrl-V / `paste`: the system clipboard's text, or the register when
    /// that is empty or unavailable.
    fn paste_clipboard(&mut self) {
        let text = match self.clipboard.get() {
            Some(text) if !text.is_empty() => text,
            _ => self.register.clone(),
        };
        if text.is_empty() {
            self.editor_mut().set_message("Nothing to paste".to_string());
        } else {
            self.editor_mut().insert_text(&normalize_breaks(&text));
        }
    }

    /// Text that arrived in one piece (the terminal's bracketed paste). In
    /// the editor it is inserted whole as one undo step; in an open prompt it
    /// goes into the prompt without line breaks (so it can never run a
    /// command); the views and help ignore it.
    pub fn paste(&mut self, text: &str) {
        if self.mode != Mode::Editor {
            return;
        }
        // Like any key: the startup hint goes, pending warnings are withdrawn.
        if std::mem::take(&mut self.hint_pending) {
            for editor in &mut self.editors {
                if editor.message() == "F1: help" {
                    editor.set_message(String::new());
                }
            }
        }
        self.quit_armed = false;
        self.close_armed = false;
        self.editor_mut().set_message(String::new());
        if self.editor_mut().paste_into_prompt(text) || text.is_empty() {
            return;
        }
        self.editor_mut().insert_text(&normalize_breaks(text));
    }

    /// After the editor handled a key: runs the command the key accepted, if any.
    fn finish_key(&mut self, flow: Flow, now: SystemTime) -> Flow {
        if let Some(term) = self.editor_mut().take_search() {
            self.search = term;
        }
        match self.editor_mut().take_command() {
            Some(text) => self.run_command(&text, now),
            None => flow,
        }
    }

    /// Runs a command typed in the Ctrl-X prompt; each does what its key does.
    /// A parse error becomes the current editor's message.
    pub fn run_command(&mut self, text: &str, now: SystemTime) -> Flow {
        let command = match command::parse(text) {
            Ok(command) => command,
            Err(message) => {
                if !message.is_empty() {
                    self.editor_mut().set_message(message);
                }
                return Flow::Continue;
            }
        };
        match command {
            Command::Save => self.editor_mut().save(),
            Command::SaveAs(path) => self.save_as(&path),
            Command::Goto(n) => self.editor_mut().go_to_line(n),
            Command::Lines(setting) => self.line_numbers = setting.unwrap_or(!self.line_numbers),
            Command::Next => self.cycle(true),
            Command::Prev => self.cycle(false),
            Command::Close => self.close_current(),
            Command::Buffers => self.enter_buffer_list(),
            Command::Tree => self.enter_file_tree(),
            Command::Git => self.enter_git_view(now),
            Command::Help => self.enter_help(),
            Command::Undo => self.editor_mut().undo(),
            Command::Redo => self.editor_mut().redo(),
            Command::Copy => self.copy(),
            Command::Cut => self.cut(),
            Command::Paste => self.paste_clipboard(),
            Command::SelectAll => self.editor_mut().select_all(),
            Command::Quit => return self.quit(),
            Command::WriteQuit => {
                self.editor_mut().save();
                // A missing file name or a failed save leaves the buffer (and
                // its message) as it is, and ted stays open.
                if self.editor().buffer().path().is_some() && !self.editor().buffer().is_modified() {
                    return self.quit();
                }
            }
        }
        Flow::Continue
    }

    /// `saveas`: writes the current buffer to a new file. Never overwrites an
    /// existing path (a dangling symlink counts) and refuses a file that
    /// another buffer has open.
    fn save_as(&mut self, rel: &str) {
        let path = self.location.join(rel);
        let name = path.file_name().map_or_else(|| rel.to_string(), |n| n.to_string_lossy().into_owned());
        let open_at = self.position_of(&path);
        if open_at.is_some_and(|i| i != self.current) {
            self.editor_mut().set_message(format!("Already open: {name}"));
        } else if open_at.is_some() || std::fs::symlink_metadata(&path).is_ok() {
            self.editor_mut().set_message(format!("Exists: {name}"));
        } else {
            self.editor_mut().save_as(path);
        }
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
            Mode::Buffers | Mode::Help => {}
        }
    }

    pub fn screen(&mut self, width: usize, height: usize, now: SystemTime) -> Screen {
        let n = self.editors.len();
        let tag = if n > 1 { format!("[{}/{}]", self.current + 1, n) } else { String::new() };
        self.editor_mut().set_tag(tag);
        let line_numbers = self.line_numbers;
        self.editor_mut().set_line_numbers(line_numbers);
        if self.mode == Mode::Help {
            return self.help.render(width, height);
        }
        match (self.mode, &self.git_view, &self.file_tree, &self.buffer_list) {
            (Mode::GitView, Some(view), _, _) => view.render(width, height, now),
            (Mode::Files, _, Some(tree), _) => tree.render(width, height),
            (Mode::Buffers, _, _, Some(list)) => {
                let entries: Vec<(String, bool)> = self.editors.iter().map(entry).collect();
                list.render(&entries, self.current, width, height)
            }
            _ => editor_screen(self.editor_mut().view(width, height)),
        }
    }

    fn enter_git_view(&mut self, now: SystemTime) {
        let view = self.git_view.get_or_insert_with(GitView::new);
        view.refresh(self.git.as_ref(), now);
        self.mode = Mode::GitView;
    }

    fn enter_buffer_list(&mut self) {
        self.close_picker();
        match &mut self.buffer_list {
            Some(list) => list.reset(self.current),
            None => self.buffer_list = Some(BufferList::new(self.current)),
        }
        self.mode = Mode::Buffers;
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
            Mode::Buffers => {
                if let Some(list) = &mut self.buffer_list {
                    list.set_message(message);
                }
            }
            Mode::Help => self.help.set_message(message),
        }
    }

    /// Opens `rel` (relative to `location`) in the editor and returns to it.
    /// A refusal is shown in the view that asked, which stays showing.
    fn open(&mut self, rel: &str) {
        let path = self.location.join(rel);
        if let Some(i) = self.position_of(&path) {
            self.set_current(i);
            self.mode = Mode::Editor;
            // A buffer with edits keeps them and is not read again. An
            // unedited one is refreshed from disk, unless the disk text is
            // what it already holds (history and cursor stay); if the read
            // fails the old copy stays and the error shows in the editor.
            if !self.editors[i].buffer().is_modified() {
                match self.load(&path) {
                    Ok(buffer) if buffer.text() == self.editors[i].buffer().text() => {}
                    Ok(buffer) => self.editors[i] = Editor::new(buffer),
                    Err(message) => self.editors[i].set_message(message),
                }
            }
            return;
        }
        match self.load(&path) {
            Ok(buffer) => {
                self.add_buffer(buffer);
                self.mode = Mode::Editor;
            }
            Err(message) => self.set_view_message(message),
        }
    }

    /// Index of the open buffer holding the same file as `path`.
    fn position_of(&self, path: &Path) -> Option<usize> {
        self.editors.iter().position(|e| e.buffer().path().is_some_and(|p| same_file(p, path)))
    }

    /// Makes `buffer` current: replaces a lone pristine unnamed buffer, or appends.
    fn add_buffer(&mut self, buffer: Buffer) {
        let only = &self.editors[0];
        let pristine = self.editors.len() == 1
            && only.buffer().path().is_none()
            && !only.buffer().is_modified()
            && !only.buffer().can_undo()
            && !only.buffer().can_redo()
            && only.buffer().text().is_empty();
        if pristine {
            self.editors[0] = Editor::new(buffer);
            self.current = 0;
        } else {
            self.editors[self.current].clear_selection();
            self.editors.push(Editor::new(buffer));
            self.current = self.editors.len() - 1;
        }
    }

    /// Reads `path` from disk.
    fn load(&self, path: &Path) -> Result<Buffer, String> {
        // `Buffer::open` treats a missing file as new, and a later save
        // would recreate it. `metadata` follows symlinks, so a dangling
        // link or an unreadable parent is refused too.
        let Ok(meta) = std::fs::metadata(path) else {
            return Err("File no longer exists".to_string());
        };
        // Reading a FIFO or a device can block forever (or never end).
        if !meta.is_file() {
            return Err("Not a regular file".to_string());
        }
        if meta.len() > MAX_OPEN_BYTES {
            return Err("File too large".to_string());
        }
        Buffer::open(path).map_err(|e| format!("Open failed: {e}"))
    }
}

/// Pasted text with CRLF and lone CR turned into `\n` (tabs and everything
/// else stay as they are).
fn normalize_breaks(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\r') {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}

/// A buffer's row in the buffer list: file name (or `[No Name]`), modified.
fn entry(editor: &Editor) -> (String, bool) {
    let buffer = editor.buffer();
    let name = buffer
        .path()
        .and_then(|p| p.file_name())
        .map_or_else(|| "[No Name]".to_string(), |n| n.to_string_lossy().into_owned());
    (name, buffer.is_modified())
}

/// Whether two paths name the same file (which need not exist yet).
fn same_file(a: &Path, b: &Path) -> bool {
    resolve(a) == resolve(b)
}

/// The canonical path; for a file that does not exist, the canonical parent
/// plus the file name, or failing that the absolute path with `.` and `..`
/// folded away lexically.
fn resolve(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut folded = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                folded.pop();
            }
            other => folded.push(other),
        }
    }
    match (folded.parent(), folded.file_name()) {
        (Some(parent), Some(name)) => std::fs::canonicalize(parent).map_or(folded.clone(), |p| p.join(name)),
        _ => folded,
    }
}

/// The editor's frame as a `Screen`: consecutive chars of one style share a
/// span, so a plain row is one `Normal` span, or a `LineNo` gutter span
/// followed by one `Normal` span when line numbers are on.
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
