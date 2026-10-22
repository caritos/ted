use std::fs;
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
    modified: bool,
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
            modified: false,
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
        self.modified = false;
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

    /// True after any edit since the last open or save. There is no undo, so
    /// typing and then deleting a character still counts as modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn insert_char(&mut self, c: char) {
        self.rope.insert_char(self.cursor_index(), c);
        self.col += 1;
        self.goal_col = self.col;
        self.modified = true;
    }

    pub fn insert_newline(&mut self) {
        self.rope.insert_char(self.cursor_index(), '\n');
        self.row += 1;
        self.col = 0;
        self.goal_col = 0;
        self.modified = true;
    }

    pub fn backspace(&mut self) {
        let index = self.cursor_index();
        if index == 0 {
            return;
        }
        if self.col > 0 {
            self.rope.remove(index - 1..index);
            self.col -= 1;
        } else {
            let prev = self.row - 1;
            let break_len = break_len(self.rope.line(prev));
            let prev_len = self.line_len(prev);
            self.rope.remove(index - break_len..index);
            self.row = prev;
            self.col = prev_len;
        }
        self.goal_col = self.col;
        self.modified = true;
    }

    pub fn delete(&mut self) {
        let index = self.cursor_index();
        if index >= self.rope.len_chars() {
            return;
        }
        let end = if self.col < self.line_len(self.row) {
            index + 1
        } else {
            index + break_len(self.rope.line(self.row))
        };
        self.rope.remove(index..end);
        self.modified = true;
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
        self.goal_col = self.col;
    }

    pub fn move_right(&mut self) {
        if self.col < self.line_len(self.row) {
            self.col += 1;
        } else if self.row + 1 < self.line_count() {
            self.row += 1;
            self.col = 0;
        }
        self.goal_col = self.col;
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.goal_col.min(self.line_len(self.row));
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.line_count() {
            self.row += 1;
            self.col = self.goal_col.min(self.line_len(self.row));
        }
    }

    fn cursor_index(&self) -> usize {
        self.rope.line_to_char(self.row) + self.col
    }

    fn line_len(&self, row: usize) -> usize {
        let line = self.rope.line(row);
        line.len_chars() - break_len(line)
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
