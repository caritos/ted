//! The editor plus the git view, and which of the two is showing.

use std::path::PathBuf;
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;
use crate::editor::{Editor, Flow, View};
use crate::git::Git;
use crate::gitview::{GitAction, GitView};
use crate::screen::{Screen, Span, Style};

pub struct App {
    editor: Editor,
    /// Directory the git view looks at; opened paths are relative to it.
    location: PathBuf,
    git: Box<dyn Git>,
    /// Created on the first Ctrl-G and kept, so the baseline survives.
    git_view: Option<GitView>,
    in_git_view: bool,
}

impl App {
    pub fn new(editor: Editor, location: PathBuf, git: Box<dyn Git>) -> Self {
        App { editor, location, git, git_view: None, in_git_view: false }
    }

    pub fn in_git_view(&self) -> bool {
        self.in_git_view
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: SystemTime) -> Flow {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if !self.in_git_view {
            if ctrl && key.code == KeyCode::Char('g') {
                self.enter_git_view(now);
                return Flow::Continue;
            }
            return self.editor.handle_key(key);
        }

        // Ctrl-Q follows the editor's quit rules; its warning shows here.
        if ctrl && key.code == KeyCode::Char('q') {
            let flow = self.editor.handle_key(key);
            if let Some(view) = &mut self.git_view {
                view.set_message(self.editor.message().to_string());
            }
            return flow;
        }
        // Any other key cancels a pending quit warning, as in the editor.
        self.editor.cancel_quit();

        let Some(view) = &mut self.git_view else {
            self.in_git_view = false;
            return Flow::Continue;
        };
        match view.handle_key(key, self.git.as_ref(), now) {
            GitAction::None => {}
            GitAction::Back => self.in_git_view = false,
            GitAction::Open(rel) => self.open(&rel),
        }
        Flow::Continue
    }

    /// Polls git while the git view is showing; does nothing otherwise.
    pub fn tick(&mut self, now: SystemTime) {
        if !self.in_git_view {
            return;
        }
        if let Some(view) = &mut self.git_view {
            view.refresh(self.git.as_ref(), now);
        }
    }

    pub fn screen(&mut self, width: usize, height: usize, now: SystemTime) -> Screen {
        match &self.git_view {
            Some(view) if self.in_git_view => view.render(width, height, now),
            _ => editor_screen(self.editor.view(width, height)),
        }
    }

    fn enter_git_view(&mut self, now: SystemTime) {
        let view = self.git_view.get_or_insert_with(GitView::new);
        view.refresh(self.git.as_ref(), now);
        self.in_git_view = true;
    }

    /// Opens `rel` in the editor, unless that would discard unsaved edits.
    fn open(&mut self, rel: &str) {
        let Some(view) = &mut self.git_view else { return };
        if self.editor.buffer().is_modified() {
            view.set_message("Unsaved changes: save or quit first".to_string());
            return;
        }
        match Buffer::open(&self.location.join(rel)) {
            Ok(buffer) => {
                self.editor.load(buffer);
                self.in_git_view = false;
            }
            Err(e) => view.set_message(format!("Open failed: {e}")),
        }
    }
}

/// The editor's frame as a `Screen`: one plain span per row.
fn editor_screen(view: View) -> Screen {
    Screen {
        rows: view
            .lines
            .into_iter()
            .map(|text| vec![Span { text, style: Style::Normal }])
            .collect(),
        status: view.status,
        message: view.message,
        cursor: Some(view.cursor),
    }
}

#[cfg(test)]
mod tests;
