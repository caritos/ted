use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;

const TAB_WIDTH: usize = 4;

#[derive(Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// Everything the terminal layer needs to draw one frame.
#[derive(Debug)]
pub struct View {
    /// Exactly the visible text rows, already clipped to the width.
    pub lines: Vec<String>,
    pub status: String,
    pub message: String,
    /// Cursor position on screen as (row, column) within `lines`.
    pub cursor: (usize, usize),
}

pub struct Editor {
    buffer: Buffer,
    message: String,
    quit_armed: bool,
    scroll_row: usize,
    scroll_col: usize,
}

impl Editor {
    pub fn new(buffer: Buffer) -> Self {
        Editor {
            buffer,
            message: String::new(),
            quit_armed: false,
            scroll_row: 0,
            scroll_col: 0,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Flow {
        let quit_was_armed = std::mem::take(&mut self.quit_armed);
        self.message.clear();

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Char('q') if ctrl => {
                if !self.buffer.is_modified() || quit_was_armed {
                    return Flow::Quit;
                }
                self.message =
                    "Unsaved changes! Press Ctrl-Q again to quit without saving.".to_string();
                self.quit_armed = true;
            }
            KeyCode::Char(c) if !ctrl && !alt => self.buffer.insert_char(c),
            KeyCode::Enter => self.buffer.insert_newline(),
            KeyCode::Backspace => self.buffer.backspace(),
            KeyCode::Delete => self.buffer.delete(),
            KeyCode::Tab => {
                for _ in 0..TAB_WIDTH {
                    self.buffer.insert_char(' ');
                }
            }
            KeyCode::Left => self.buffer.move_left(),
            KeyCode::Right => self.buffer.move_right(),
            KeyCode::Up => self.buffer.move_up(),
            KeyCode::Down => self.buffer.move_down(),
            _ => {}
        }
        Flow::Continue
    }

    /// Lays out one frame for a `width` x `height` terminal: text rows, then a
    /// status bar, then a message row. Scrolls just enough to show the cursor.
    pub fn view(&mut self, width: usize, height: usize) -> View {
        let text_rows = height.saturating_sub(2);
        let (row, col) = self.buffer.cursor();

        if text_rows > 0 {
            if row < self.scroll_row {
                self.scroll_row = row;
            } else if row >= self.scroll_row + text_rows {
                self.scroll_row = row + 1 - text_rows;
            }
        }
        if width > 0 {
            if col < self.scroll_col {
                self.scroll_col = col;
            } else if col >= self.scroll_col + width {
                self.scroll_col = col + 1 - width;
            }
        }

        let lines = (0..text_rows)
            .map(|i| {
                let r = self.scroll_row + i;
                if r < self.buffer.line_count() {
                    clip(&self.buffer.line(r), self.scroll_col, width)
                } else {
                    clip("~", 0, width)
                }
            })
            .collect();

        View {
            lines,
            status: self.status_line(width),
            message: clip(&self.message, 0, width),
            cursor: (row - self.scroll_row, col - self.scroll_col),
        }
    }

    fn save(&mut self) {
        if self.buffer.path().is_none() {
            self.message = "No file name: start ted with a path to save.".to_string();
            return;
        }
        match self.buffer.save() {
            Ok(()) => self.message = format!("Saved {} lines", self.buffer.line_count()),
            Err(e) => self.message = format!("Save failed: {e}"),
        }
    }

    fn status_line(&self, width: usize) -> String {
        let name = self
            .buffer
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[No Name]".to_string());
        let marker = if self.buffer.is_modified() { " [+]" } else { "" };
        let left = format!(" {name}{marker}");
        let (row, col) = self.buffer.cursor();
        let right = format!("{}:{} ", row + 1, col + 1);

        let right_len = right.chars().count();
        if width < right_len {
            return right.chars().take(width).collect();
        }
        let left_max = width - right_len;
        let mut status: String = left.chars().take(left_max).collect();
        let pad = left_max - status.chars().count();
        status.extend(std::iter::repeat_n(' ', pad));
        status.push_str(&right);
        status
    }
}

/// `width` chars of `text` starting at char `skip`.
fn clip(text: &str, skip: usize, width: usize) -> String {
    text.chars().skip(skip).take(width).collect()
}

#[cfg(test)]
mod tests;
