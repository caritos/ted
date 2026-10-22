use std::fs;
use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

use ropey::{Rope, RopeSlice};

/// Text plus a cursor. Columns count `char`s, not display cells or graphemes.
#[derive(Debug)]
pub struct Buffer {
    rope: Rope,
    row: usize,
    col: usize,
    /// Column to return to when moving vertically across shorter lines.
    goal_col: usize,
    path: Option<PathBuf>,
    /// Applied edits, oldest first; its length is the history position.
    undo: VecDeque<Edit>,
    /// Undone edits, the next one to redo last.
    redo: Vec<Edit>,
    /// History position at the last open or save; `None` once unreachable.
    clean: Option<usize>,
    /// Whether the next typing/deleting edit may merge into the last one.
    merge_ok: bool,
    /// Lowest row whose text changed since the last `take_dirty_row`.
    dirty_row: Option<usize>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self::from_text("")
    }

    pub fn from_text(text: &str) -> Self {
        Buffer {
            rope: Rope::from_str(text),
            row: 0,
            col: 0,
            goal_col: 0,
            path: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            clean: Some(0),
            merge_ok: false,
            dirty_row: Some(0),
        }
    }

    /// Loads `path`. A missing file gives an empty buffer that will create it
    /// on save; any other failure (including non-UTF-8 content) is an error.
    pub fn open(path: &Path) -> io::Result<Self> {
        let text = match fs::read(path) {
            Ok(bytes) => String::from_utf8(bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut buffer = Self::from_text(&text);
        buffer.path = Some(path.to_path_buf());
        Ok(buffer)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        write_atomic(&path, &self.text())?;
        self.mark_clean();
        Ok(())
    }

    /// Writes the text to `path` (atomically). Only on success does the
    /// buffer take the new path and become unmodified.
    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        write_new(&path, &self.text())?;
        self.path = Some(path);
        self.mark_clean();
        Ok(())
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// The text of `row` without its line break; empty if out of range.
    pub fn line(&self, row: usize) -> String {
        if row >= self.line_count() {
            return String::new();
        }
        self.rope.line(row).slice(..self.line_len(row)).to_string()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// True when the history position differs from the one at the last open or
    /// save, so typing and then undoing it is unmodified again. After a new
    /// edit cuts off (or the cap drops) the saved state it stays modified.
    pub fn is_modified(&self) -> bool {
        self.clean != Some(self.undo.len())
    }

    fn mark_clean(&mut self) {
        self.clean = Some(self.undo.len());
        self.merge_ok = false;
    }

    /// Undoes the last edit step. False when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop_back() else { return false };
        self.apply(edit.index, edit.inserted_len, &edit.removed_text());
        self.place_cursor(edit.before);
        self.goal_col = self.col;
        self.redo.push(edit);
        self.merge_ok = false;
        true
    }

    /// Redoes the last undone step. False when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        self.apply(edit.index, edit.removed.len(), &edit.inserted);
        self.place_cursor(edit.after);
        self.goal_col = self.col;
        self.undo.push_back(edit);
        self.merge_ok = false;
        true
    }

    /// Replaces `remove_len` chars at `index` with `insert`, marking every row
    /// that may have changed (one above when line breaks are involved).
    fn apply(&mut self, index: usize, remove_len: usize, insert: &str) {
        let removed: Vec<char> = self.rope.slice(index..index + remove_len).chars().collect();
        let breaks = removed.iter().chain(insert.chars().collect::<Vec<_>>().iter()).any(|&c| is_break(c));
        let before_row = self.rope.char_to_line(index);
        self.rope.remove(index..index + remove_len);
        self.rope.insert(index, insert);
        let after_row = self.rope.char_to_line(index.min(self.rope.len_chars()));
        let row = before_row.min(after_row);
        self.mark_dirty(if breaks { row.saturating_sub(1) } else { row });
    }

    /// Records a performed edit: merges it into the last step when the merge
    /// rules allow, otherwise pushes a new step. Clears the redo branch.
    fn record(&mut self, edit: Edit) {
        if !self.redo.is_empty() {
            if self.clean.is_some_and(|c| c > self.undo.len()) {
                self.clean = None;
            }
            self.redo.clear();
        }
        let len = self.undo.len();
        if self.merge_ok
            && self.clean != Some(len)
            && let Some(last) = self.undo.back_mut()
            && last.merge(&edit)
        {
            return;
        }
        self.undo.push_back(edit);
        if self.undo.len() > MAX_EDITS {
            self.undo.pop_front();
            self.clean = self.clean.and_then(|c| c.checked_sub(1));
        }
        self.merge_ok = true;
    }

    /// The lowest row changed since the last call (a new buffer counts as
    /// changed from row 0), or `None`. Text at and below that row may differ;
    /// rows above it are untouched.
    pub(crate) fn take_dirty_row(&mut self) -> Option<usize> {
        self.dirty_row.take()
    }

    /// Records that `row` changed (keeping the lowest such row).
    fn mark_dirty(&mut self, row: usize) {
        self.dirty_row = Some(self.dirty_row.map_or(row, |d| d.min(row)));
    }

    pub fn insert_char(&mut self, c: char) {
        self.mark_dirty(self.row);
        let index = self.cursor_index();
        self.rope.insert_char(index, c);
        self.col += 1;
        self.goal_col = self.col;
        self.record(Edit::new(index, "", &c.to_string(), index, index + 1, false));
    }

    pub fn insert_newline(&mut self) {
        // Splitting a line: be conservative and include the row above.
        self.mark_dirty(self.row.saturating_sub(1));
        let index = self.cursor_index();
        self.rope.insert_char(index, '\n');
        // A \n inserted after a lone \r joins with it into one break, so the
        // row cannot simply be incremented.
        self.place_cursor(index + 1);
        self.goal_col = self.col;
        self.record(Edit::new(index, "", "\n", index, index + 1, false));
    }

    pub fn backspace(&mut self) {
        let index = self.cursor_index();
        if index == 0 {
            return;
        }
        let start = if self.col > 0 {
            self.mark_dirty(self.row);
            index - 1
        } else {
            let prev = self.row - 1;
            // Joining with the line above: that row changes.
            self.mark_dirty(prev);
            index - break_len(self.rope.line(prev))
        };
        let removed = self.rope.slice(start..index).to_string();
        self.rope.remove(start..index);
        self.place_cursor(start);
        self.goal_col = self.col;
        self.record(Edit::new(start, &removed, "", index, start, true));
    }

    pub fn delete(&mut self) {
        let index = self.cursor_index();
        if index >= self.rope.len_chars() {
            return;
        }
        let end = if self.col < self.line_len(self.row) {
            self.mark_dirty(self.row);
            index + 1
        } else {
            // Joining the next line onto this one: include the row above.
            self.mark_dirty(self.row.saturating_sub(1));
            index + break_len(self.rope.line(self.row))
        };
        let removed = self.rope.slice(index..end).to_string();
        self.rope.remove(index..end);
        self.place_cursor(index);
        self.record(Edit::new(index, &removed, "", index, index, false));
    }

    pub fn move_left(&mut self) {
        self.merge_ok = false;
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
        self.goal_col = self.col;
    }

    pub fn move_right(&mut self) {
        self.merge_ok = false;
        if self.col < self.line_len(self.row) {
            self.col += 1;
        } else if self.row + 1 < self.line_count() {
            self.row += 1;
            self.col = 0;
        }
        self.goal_col = self.col;
    }

    pub fn move_up(&mut self) {
        self.merge_ok = false;
        if self.row > 0 {
            self.row -= 1;
            self.col = self.goal_col.min(self.line_len(self.row));
        }
    }

    pub fn move_down(&mut self) {
        self.merge_ok = false;
        if self.row + 1 < self.line_count() {
            self.row += 1;
            self.col = self.goal_col.min(self.line_len(self.row));
        }
    }

    /// Puts the cursor at char `index`, whatever the edit did to the line
    /// breaks around it (a \r and \n made adjacent by an edit become one break).
    fn place_cursor(&mut self, index: usize) {
        let index = index.min(self.rope.len_chars());
        self.row = self.rope.char_to_line(index);
        self.col = (index - self.rope.line_to_char(self.row)).min(self.line_len(self.row));
    }

    pub fn move_home(&mut self) {
        self.merge_ok = false;
        self.col = 0;
        self.goal_col = 0;
    }

    pub fn move_end(&mut self) {
        self.merge_ok = false;
        self.col = self.line_len(self.row);
        self.goal_col = self.col;
    }

    /// Moves up `rows` lines (stopping at the first), keeping the goal column.
    pub fn move_up_by(&mut self, rows: usize) {
        self.merge_ok = false;
        self.row = self.row.saturating_sub(rows);
        self.col = self.goal_col.min(self.line_len(self.row));
    }

    /// Moves down `rows` lines (stopping at the last), keeping the goal column.
    pub fn move_down_by(&mut self, rows: usize) {
        self.merge_ok = false;
        self.row = (self.row + rows).min(self.line_count() - 1);
        self.col = self.goal_col.min(self.line_len(self.row));
    }

    /// Finds `query` within single lines. Smart case: case-insensitive unless
    /// the query has an uppercase char. Forward: the first match starting at
    /// or after `from` (strictly after unless `include_from`), wrapping to the
    /// top. Backward: the last match starting strictly before `from`, wrapping
    /// to the bottom. A wrapped search may land on the match at `from` itself.
    /// Never changes the buffer or the cursor.
    pub fn find(
        &self,
        query: &str,
        from: (usize, usize),
        forward: bool,
        include_from: bool,
    ) -> Option<Found> {
        #[cfg(test)]
        FINDS.with(|c| c.set(c.get() + 1));
        if query.is_empty() {
            return None;
        }
        let exact = query.chars().any(char::is_uppercase);
        let needle: Vec<char> = query.chars().map(|c| fold(c, exact)).collect();
        let rows = self.line_count();
        // Both buffers are reused for every row, so a scan allocates once.
        let mut scratch = Scratch::default();
        let mut scan = |line: RopeSlice| -> Vec<usize> { row_matches(line, query, &needle, exact, &mut scratch).to_vec() };
        let found = |row: usize, col: usize, wrapped: bool| Some(Found { row, col, wrapped });
        let (from_row, from_col) = from;
        let last = from_row.min(rows - 1);
        // Each pass walks the rope's lines once instead of looking rows up.
        if forward {
            for (i, line) in self.rope.lines_at(from_row.min(rows)).enumerate() {
                let row = from_row + i;
                let hit = scan(line)
                    .into_iter()
                    .find(|&c| row > from_row || c > from_col || (include_from && c == from_col));
                if let Some(col) = hit {
                    return found(row, col, false);
                }
            }
            for (row, line) in self.rope.lines().take(last + 1).enumerate() {
                if let Some(&col) = scan(line).first() {
                    return found(row, col, true);
                }
            }
        } else {
            for (i, line) in self.rope.lines_at(last + 1).reversed().enumerate() {
                let row = last - i;
                let hit = scan(line).into_iter().rev().find(|&c| row < from_row || c < from_col);
                if let Some(col) = hit {
                    return found(row, col, false);
                }
            }
            for (i, line) in self.rope.lines_at(rows).reversed().take(rows - last).enumerate() {
                if let Some(&col) = scan(line).last() {
                    return found(rows - 1 - i, col, true);
                }
            }
        }
        None
    }

    /// Puts the cursor at the nearest valid position to (`row`, `col`).
    /// Not an edit: leaves the modified flag and dirty row alone.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.merge_ok = false;
        self.row = row.min(self.line_count() - 1);
        self.col = col.min(self.line_len(self.row));
        self.goal_col = self.col;
    }

    /// Moves to column 0 of the 1-based `line`, clamped to the buffer (0 acts
    /// as line 1).
    pub fn go_to_line(&mut self, line_1_based: usize) {
        self.set_cursor(line_1_based.saturating_sub(1), 0);
    }

    fn cursor_index(&self) -> usize {
        self.rope.line_to_char(self.row) + self.col
    }

    fn line_len(&self, row: usize) -> usize {
        let line = self.rope.line(row);
        line.len_chars() - break_len(line)
    }
}

/// Most steps kept in the undo history; the oldest are dropped.
const MAX_EDITS: usize = 10_000;

/// One undoable step: at char `index`, the removed chars were replaced by
/// `inserted`. `before` / `after` are the cursor's char index around the step.
/// Run-merging only looks at the new piece, so a long run stays linear.
#[derive(Debug)]
struct Edit {
    index: usize,
    /// Removed chars in removal order: text order, or reversed when `rev`
    /// (Backspace runs grow backwards, so they are pushed in O(1)).
    removed: Vec<char>,
    rev: bool,
    inserted: String,
    inserted_len: usize,
    /// No line-break char removed or inserted (such a step never merges).
    plain: bool,
    before: usize,
    after: usize,
}

impl Edit {
    fn new(index: usize, removed: &str, inserted: &str, before: usize, after: usize, rev: bool) -> Self {
        let mut chars: Vec<char> = removed.chars().collect();
        if rev {
            chars.reverse();
        }
        Edit {
            index,
            plain: !chars.iter().copied().chain(inserted.chars()).any(is_break),
            removed: chars,
            rev,
            inserted: inserted.to_string(),
            inserted_len: inserted.chars().count(),
            before,
            after,
        }
    }

    /// The removed text in document order.
    fn removed_text(&self) -> String {
        if self.rev {
            self.removed.iter().rev().collect()
        } else {
            self.removed.iter().collect()
        }
    }

    /// Folds `next` (performed right after `self`) into `self` if both are
    /// part of one run of typing, Backspaces or Deletes of non-break chars.
    fn merge(&mut self, next: &Edit) -> bool {
        if self.after != next.before || !self.plain || !next.plain {
            return false;
        }
        if self.removed.is_empty() && next.removed.is_empty() {
            if self.inserted.is_empty() || self.index + self.inserted_len != next.index {
                return false;
            }
            self.inserted.push_str(&next.inserted);
            self.inserted_len += next.inserted_len;
            self.after = next.after;
            return true;
        }
        if !self.inserted.is_empty() || !next.inserted.is_empty() || self.rev != next.rev {
            return false;
        }
        if self.rev {
            if next.index + next.removed.len() != self.index {
                return false;
            }
            self.removed.extend_from_slice(&next.removed);
            self.index = next.index;
            self.after = next.after;
            true
        } else {
            if next.index != self.index {
                return false;
            }
            self.removed.extend_from_slice(&next.removed);
            true
        }
    }
}

/// Whether `c` ends a line for ropey.
fn is_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Columns where `needle` (folded) starts in the text of `line` (its break
/// excluded), ascending, left in `scratch`. An exact-case query first tests
/// the line with a plain `contains` when the line is one contiguous chunk,
/// which skips most rows without building any chars.
fn row_matches<'a>(
    line: RopeSlice,
    query: &str,
    needle: &[char],
    exact: bool,
    scratch: &'a mut Scratch,
) -> &'a [usize] {
    let Scratch { hay, cols } = scratch;
    cols.clear();
    let text = line.slice(..line.len_chars() - break_len(line));
    if exact && text.as_str().is_some_and(|s| !s.contains(query)) {
        return cols;
    }
    hay.clear();
    match text.as_str() {
        Some(s) => hay.extend(s.chars().map(|c| fold(c, exact))),
        None => hay.extend(text.chars().map(|c| fold(c, exact))),
    }
    if let (Some(&first), true) = (needle.first(), hay.len() >= needle.len()) {
        let last_start = hay.len() - needle.len();
        cols.extend((0..=last_start).filter(|&i| hay[i] == first && hay[i..i + needle.len()] == *needle));
    }
    cols
}

/// Buffers a search reuses for every row.
#[derive(Default)]
struct Scratch {
    hay: Vec<char>,
    cols: Vec<usize>,
}

#[cfg(test)]
thread_local! {
    /// Number of `Buffer::find` calls on this thread, for tests.
    pub(crate) static FINDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A search match: `col` is a char index within the line text.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Found {
    pub row: usize,
    pub col: usize,
    pub wrapped: bool,
}

/// One char for smart-case comparison; never changes the char count.
fn fold(c: char, exact: bool) -> char {
    if exact {
        c
    } else if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

/// Number of chars in the line break ending `line` (0 for the last line).
fn break_len(line: RopeSlice) -> usize {
    let n = line.len_chars();
    if n == 0 {
        return 0;
    }
    match line.char(n - 1) {
        '\n' if n >= 2 && line.char(n - 2) == '\r' => 2,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => 1,
        _ => 0,
    }
}

/// Writes via a sibling temp file and rename so a failed write never leaves a
/// half-written file. Follows symlinks and keeps the existing permissions.
/// Creates `path` with `text`, never replacing anything that exists there
/// (a file, a directory or a symlink, even a dangling one): `AlreadyExists`.
/// The text goes to a uniquely named temp file first and is hard-linked into
/// place, which fails atomically if the target appeared meanwhile.
fn write_new(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file path"))?;
    let mut tmp;
    let mut file = loop {
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".ted-tmp-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        tmp = path.with_file_name(tmp_name);
        match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => break file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let written = file.write_all(text.as_bytes()).and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| match fs::hard_link(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        // No hard links here (some file systems): claim the name with an
        // empty file, then replace it atomically.
        Err(_) => {
            fs::OpenOptions::new().write(true).create_new(true).open(path)?;
            fs::rename(&tmp, path).inspect_err(|_| {
                let _ = fs::remove_file(path);
            })
        }
    });
    let _ = fs::remove_file(&tmp);
    result
}

fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file path"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(".ted-tmp");
    let tmp = target.with_file_name(tmp_name);

    let result = (|| {
        fs::write(&tmp, text)?;
        if let Ok(meta) = fs::metadata(&target) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
