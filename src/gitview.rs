use std::collections::HashMap;
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::git::{format_age, Baseline, Change, ChangeKind, Commit, Git};
use crate::screen::{Row, Screen, Span, Style};

/// Commits listed in the baseline picker.
const PICKER_LIMIT: usize = 50;
/// Diff lines scrolled by PageUp/PageDown.
const DIFF_PAGE: usize = 20;
/// Narrowest the file list gets when there is room.
const MIN_LIST_WIDTH: usize = 12;

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

    /// Shows `message` until the next key.
    pub fn set_message(&mut self, message: String) {
        self.message = message;
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

    /// Draws the view into `height` rows in total, status and message rows
    /// included, each at most `width` characters wide.
    pub fn render(&self, width: usize, height: usize, now: SystemTime) -> Screen {
        // Status and message take the last two rows; with less room than
        // that nothing is drawn but blank rows.
        let body_h = height.saturating_sub(2);
        let mut rows: Vec<Row> = if self.picker.is_some() {
            self.picker_rows(width, body_h)
        } else if let Some(err) = &self.error {
            vec![styled(err, width, Style::Normal)]
        } else {
            self.split_rows(width, body_h, now)
        };
        rows.truncate(body_h);
        rows.resize_with(body_h, Vec::new);

        let (status, message) = if height >= 2 {
            // Padded so the reverse-video bar overwrites the whole row.
            (pad(&self.status_text(), width), clip(&self.message, width))
        } else {
            (String::new(), String::new())
        };
        Screen { rows, status, message, cursor: None }
    }

    fn status_text(&self) -> String {
        // The list is hidden in the error state, so no count either.
        if self.error.is_some() {
            return "Error".to_string();
        }
        let base = match &self.baseline {
            None | Some(Baseline::Empty) => "Baseline: (no commits)".to_string(),
            Some(Baseline::Commit(hash)) => format!("Baseline {}", clip(hash, 7)),
        };
        let n = self.entries.len();
        format!("{base}  {n} change{}", if n == 1 { "" } else { "s" })
    }

    fn picker_rows(&self, width: usize, body_h: usize) -> Vec<Row> {
        let Some(picker) = &self.picker else { return Vec::new() };
        let first = window_start(picker.selected, body_h);
        picker
            .commits
            .iter()
            .enumerate()
            .skip(first)
            .map(|(i, c)| {
                let text = format!(
                    "{}  {}  {}  [{}]",
                    c.short,
                    c.subject,
                    format_commit_time(c.unix_time),
                    c.refs.join(", ")
                );
                let style = if i == picker.selected { Style::Selected } else { Style::Normal };
                styled(&text, width, style)
            })
            .collect()
    }

    fn split_rows(&self, width: usize, body_h: usize, now: SystemTime) -> Vec<Row> {
        let list_w = (width / 3).max(MIN_LIST_WIDTH).min(width);
        let right_w = width.saturating_sub(list_w + 1);

        let selected = self.selected.as_deref().and_then(|s| self.index_of(s)).unwrap_or(0);
        let first = window_start(selected, body_h);
        let mut left: Vec<(String, Style)> = self
            .entries
            .iter()
            .enumerate()
            .skip(first)
            .map(|(i, e)| {
                let age = format_age(now.duration_since(e.time).map_or(0, |d| d.as_secs()));
                let tail = format!("  {age}");
                let room = list_w.saturating_sub(2 + tail.chars().count());
                let text = format!("{} {}{tail}", e.change.kind.letter(), clip(&e.change.path, room));
                (text, if i == selected { Style::Selected } else { Style::Normal })
            })
            .collect();
        if self.entries.is_empty() {
            left.push(("No changes since baseline".to_string(), Style::Dim));
        }
        let mut diff = styled_diff_lines(&self.diff).skip(self.diff_scroll);

        (0..body_h)
            .map(|i| {
                let (text, style) = left.get(i).cloned().unwrap_or((String::new(), Style::Normal));
                let mut row: Row = vec![Span { text: pad(&text, list_w), style }];
                if width > list_w {
                    row.push(Span { text: "|".to_string(), style: Style::Dim });
                    if let Some((line, style)) = diff.next() {
                        let text = clip(&line.replace('\t', "    "), right_w);
                        row.push(Span { text, style });
                    }
                }
                row
            })
            .collect()
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

/// First row to show so that `selected` is inside a window of `height` rows.
fn window_start(selected: usize, height: usize) -> usize {
    (selected + 1).saturating_sub(height.max(1))
}

/// Each diff line with its style. File header lines (`diff`, `index`,
/// `---`, `+++`) are only recognised before a file's first hunk; inside a
/// hunk `--- x` is a removed line and `+++x` an added one.
fn styled_diff_lines(diff: &str) -> impl Iterator<Item = (&str, Style)> {
    let mut in_header = true;
    diff.lines().map(move |line| {
        // Hunk body lines start with ' ', '+', '-' or '\', so a `diff `
        // line always begins the next file's header.
        if line.starts_with("diff ") {
            in_header = true;
        } else if line.starts_with("@@") {
            in_header = false;
        }
        (line, diff_style(line, in_header))
    })
}

fn diff_style(line: &str, in_header: bool) -> Style {
    if in_header && ["diff", "index", "---", "+++"].iter().any(|p| line.starts_with(p)) {
        Style::Dim
    } else if line.starts_with("@@") {
        Style::Hunk
    } else if line.starts_with('+') {
        Style::Added
    } else if line.starts_with('-') {
        Style::Removed
    } else {
        Style::Normal
    }
}

/// `s` safe to print: C0 controls, DEL and C1 controls become `?` so text
/// from git, file names or messages can never send escape sequences, carriage
/// returns and the like to the terminal. A tab (diff lines expand theirs
/// before this) becomes one space so the width stays one column.
fn sanitize(s: &str) -> String {
    s.chars().map(sanitize_char).collect()
}

fn sanitize_char(c: char) -> char {
    match c {
        '\t' => ' ',
        c if c.is_control() => '?',
        c => c,
    }
}

/// At most `n` characters of `s`, sanitized. Every piece of text in a row,
/// the status or the message goes through here.
fn clip(s: &str, n: usize) -> String {
    sanitize(&s.chars().take(n).collect::<String>())
}

/// `s` clipped or space-padded to exactly `n` characters.
fn pad(s: &str, n: usize) -> String {
    let mut out = clip(s, n);
    let len = out.chars().count();
    out.extend(std::iter::repeat_n(' ', n - len));
    out
}

fn styled(s: &str, width: usize, style: Style) -> Row {
    vec![Span { text: clip(s, width), style }]
}

/// "YYYY-MM-DD HH:MM" in UTC for a Unix timestamp.
pub fn format_commit_time(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant), days since 1970-01-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", secs / 3600, secs % 3600 / 60)
}

#[cfg(test)]
mod tests;
