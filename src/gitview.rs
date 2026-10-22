use std::collections::HashMap;
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::git::{Baseline, Change, ChangeKind, Commit, Git};

/// Commits listed in the baseline picker.
const PICKER_LIMIT: usize = 50;
/// Diff lines scrolled by PageUp/PageDown.
const DIFF_PAGE: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum GitAction {
    None,
    /// Leave the git view.
    Back,
    /// Open this path, relative to the location, in the editor.
    Open(String),
}

/// One row of the file list.
struct Entry {
    change: Change,
    /// File mtime, or when the file was first noticed if it has none.
    time: SystemTime,
}

/// The commit list opened with `b`.
struct Picker {
    commits: Vec<Commit>,
    selected: usize,
}

/// State of the git view: baseline, changed files, selection, diff and the
/// baseline picker. Time is always passed in, never read from the clock.
pub struct GitView {
    baseline: Option<Baseline>,
    /// Sorted newest first, then by path.
    entries: Vec<Entry>,
    /// Selection is kept by path so it survives reordering.
    selected: Option<String>,
    /// When each path was first noticed. Never pruned or cleared, so it
    /// resets only when the viewer restarts.
    first_seen: HashMap<String, SystemTime>,
    diff: String,
    diff_scroll: usize,
    /// Set when the last git call failed; the list is hidden, not emptied.
    error: Option<String>,
    message: String,
    picker: Option<Picker>,
}

impl Default for GitView {
    fn default() -> Self {
        Self::new()
    }
}

impl GitView {
    pub fn new() -> Self {
        GitView {
            baseline: None,
            entries: Vec::new(),
            selected: None,
            first_seen: HashMap::new(),
            diff: String::new(),
            diff_scroll: 0,
            error: None,
            message: String::new(),
            picker: None,
        }
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn baseline(&self) -> Option<&Baseline> {
        self.baseline.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Reloads the file list and the selected file's diff.
    pub fn refresh(&mut self, git: &dyn Git, now: SystemTime) {
        let base = match self.current_baseline(git) {
            Ok(base) => base,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let changes = match git.changes(&base) {
            Ok(changes) => changes,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };

        let mut entries: Vec<Entry> = changes
            .into_iter()
            .map(|change| {
                let seen = *self.first_seen.entry(change.path.clone()).or_insert(now);
                let time = git.modified(&change.path).unwrap_or(seen);
                Entry { change, time }
            })
            .collect();
        entries.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.change.path.cmp(&b.change.path)));
        self.entries = entries;

        let still_listed = self.selected.as_ref().is_some_and(|s| self.index_of(s).is_some());
        if !still_listed {
            self.selected = self.entries.first().map(|e| e.change.path.clone());
        }
        self.error = None;
        self.load_diff(git);
    }

    pub fn handle_key(&mut self, key: KeyEvent, git: &dyn Git, now: SystemTime) -> GitAction {
        self.message.clear();
        if self.picker.is_some() {
            self.picker_key(key, git, now);
            return GitAction::None;
        }

        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('q') if plain => return GitAction::Back,
            KeyCode::Esc => return GitAction::Back,
            KeyCode::Char('r') if plain => self.reset_to_head(git, now),
            KeyCode::Char('b') if plain => self.open_picker(git),
            _ if self.error.is_some() => {}
            KeyCode::Up => self.move_selection(git, -1),
            KeyCode::Down => self.move_selection(git, 1),
            KeyCode::PageUp => self.diff_scroll = self.diff_scroll.saturating_sub(DIFF_PAGE),
            KeyCode::PageDown => {
                let last = self.diff.lines().count().saturating_sub(1);
                self.diff_scroll = (self.diff_scroll + DIFF_PAGE).min(last);
            }
            KeyCode::Enter => return self.open_selected(),
            _ => {}
        }
        GitAction::None
    }

    /// The baseline to diff against, set to HEAD on first use or when the
    /// remembered commit is gone.
    fn current_baseline(&mut self, git: &dyn Git) -> Result<Baseline, String> {
        if let Some(base) = &self.baseline {
            if git.baseline_exists(base) {
                return Ok(base.clone());
            }
            self.message = "Baseline commit is gone; reset to HEAD".to_string();
        }
        let head = git.head().map_err(|e| e.to_string())?;
        self.baseline = Some(head.clone());
        Ok(head)
    }

    fn reset_to_head(&mut self, git: &dyn Git, now: SystemTime) {
        match git.head() {
            Ok(head) => {
                self.baseline = Some(head);
                self.refresh(git, now);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn open_picker(&mut self, git: &dyn Git) {
        match git.log(PICKER_LIMIT) {
            Ok(commits) => {
                if commits.is_empty() {
                    self.message = "No commits yet".to_string();
                }
                self.picker = Some(Picker { commits, selected: 0 });
            }
            Err(e) => self.message = format!("Could not list commits: {e}"),
        }
    }

    fn picker_key(&mut self, key: KeyEvent, git: &dyn Git, now: SystemTime) {
        let Some(picker) = &mut self.picker else { return };
        let last = picker.commits.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => self.picker = None,
            KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::Down => picker.selected = (picker.selected + 1).min(last),
            KeyCode::Enter => {
                if let Some(commit) = picker.commits.get(picker.selected) {
                    let short = commit.short.clone();
                    self.baseline = Some(Baseline::Commit(commit.hash.clone()));
                    self.picker = None;
                    self.refresh(git, now);
                    if self.message.is_empty() {
                        self.message = format!("Baseline set to {short}");
                    }
                }
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, git: &dyn Git, delta: isize) {
        let Some(current) = self.selected.as_deref().and_then(|s| self.index_of(s)) else {
            return;
        };
        let last = self.entries.len() - 1;
        let next = current.saturating_add_signed(delta).min(last);
        if next != current {
            self.selected = Some(self.entries[next].change.path.clone());
            self.load_diff(git);
        }
    }

    fn open_selected(&mut self) -> GitAction {
        let Some(entry) = self.selected.as_deref().and_then(|s| self.index_of(s)).map(|i| &self.entries[i])
        else {
            return GitAction::None;
        };
        if entry.change.kind == ChangeKind::Deleted {
            self.message = format!("{} was deleted; nothing to open", entry.change.path);
            return GitAction::None;
        }
        GitAction::Open(entry.change.path.clone())
    }

    /// Fetches the selected file's diff; the scroll offset is kept unless
    /// the text changed.
    fn load_diff(&mut self, git: &dyn Git) {
        let text = match (self.selected_change(), &self.baseline) {
            (Some(change), Some(base)) => match git.diff(base, change) {
                Ok(text) => text,
                Err(e) => format!("Diff failed: {e}"),
            },
            _ => String::new(),
        };
        if text != self.diff {
            self.diff = text;
            self.diff_scroll = 0;
        }
    }

    fn selected_change(&self) -> Option<&Change> {
        let i = self.index_of(self.selected.as_deref()?)?;
        Some(&self.entries[i].change)
    }

    fn index_of(&self, path: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.change.path == path)
    }

    #[cfg(test)]
    pub(crate) fn paths(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.change.path.as_str()).collect()
    }

    #[cfg(test)]
    pub(crate) fn entry_time(&self, path: &str) -> Option<SystemTime> {
        self.index_of(path).map(|i| self.entries[i].time)
    }

    #[cfg(test)]
    pub(crate) fn diff(&self) -> &str {
        &self.diff
    }

    #[cfg(test)]
    pub(crate) fn diff_scroll(&self) -> usize {
        self.diff_scroll
    }

    #[cfg(test)]
    pub(crate) fn picker_open(&self) -> bool {
        self.picker.is_some()
    }

    #[cfg(test)]
    pub(crate) fn picker_commits(&self) -> &[Commit] {
        self.picker.as_ref().map_or(&[], |p| &p.commits)
    }

    /// Index of the highlighted commit; `None` when closed or empty.
    #[cfg(test)]
    pub(crate) fn picker_selected(&self) -> Option<usize> {
        self.picker.as_ref().filter(|p| !p.commits.is_empty()).map(|p| p.selected)
    }
}

#[cfg(test)]
mod tests;
