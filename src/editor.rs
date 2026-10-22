use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::Buffer;
use crate::highlight::{highlight_line, Language, LineState, Token};
use crate::screen::Style;

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
    /// One style per char of each entry of `lines`.
    pub styles: Vec<Vec<Style>>,
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
    /// Highlight state at the start of each line (index = row), valid for
    /// every entry; extended lazily and truncated at the first edited row.
    line_states: Vec<LineState>,
    /// Language `line_states` belongs to; a change clears the cache.
    highlighted: Option<Language>,
    /// Lines the cache has walked (test-only instrumentation).
    #[cfg(test)]
    walked_lines: usize,
}

impl Editor {
    pub fn new(buffer: Buffer) -> Self {
        Editor {
            buffer,
            message: String::new(),
            quit_armed: false,
            scroll_row: 0,
            scroll_col: 0,
            line_states: Vec::new(),
            highlighted: None,
            #[cfg(test)]
            walked_lines: 0,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces the buffer and starts it fresh: top-left scroll, no message,
    /// no pending quit warning, no highlight cache.
    pub fn load(&mut self, buffer: Buffer) {
        self.buffer = buffer;
        self.line_states = Vec::new();
        self.highlighted = None;
        self.message.clear();
        self.quit_armed = false;
        self.scroll_row = 0;
        self.scroll_col = 0;
    }

    /// Drops a pending Ctrl-Q warning and its message, as any other editor
    /// key would. Other messages are kept.
    pub fn cancel_quit(&mut self) {
        if std::mem::take(&mut self.quit_armed) {
            self.message.clear();
        }
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

        let lines: Vec<String> = (0..text_rows)
            .map(|i| {
                let r = self.scroll_row + i;
                if r < self.buffer.line_count() {
                    clip(&self.buffer.line(r), self.scroll_col, width)
                } else {
                    clip("~", 0, width)
                }
            })
            .collect();
        let styles = self.styles(&lines, width);

        View {
            lines,
            styles,
            status: self.status_line(width),
            message: clip(&self.message, 0, width),
            cursor: (row - self.scroll_row, col - self.scroll_col),
        }
    }

    /// One style per displayed char of `lines` (the clipped visible rows).
    /// Multi-line state (block comments, strings) at the top of the viewport
    /// comes from `line_states`, which is cut back to the first edited row
    /// and then extended only as far as the first visible row.
    fn styles(&mut self, lines: &[String], width: usize) -> Vec<Vec<Style>> {
        let language = self.buffer.path().and_then(Language::from_path);
        let dirty = self.buffer.take_dirty_row();
        if language != self.highlighted {
            self.line_states = Vec::new();
            self.highlighted = language;
        } else if let Some(row) = dirty {
            // Conservative: the state at the start of `row` is recomputed too.
            self.line_states.truncate(row);
        }
        let Some(language) = language.filter(|_| !lines.is_empty()) else {
            return lines.iter().map(|l| vec![Style::Normal; l.chars().count()]).collect();
        };

        if self.line_states.is_empty() {
            self.line_states.push(LineState::default());
        }
        while self.line_states.len() <= self.scroll_row {
            let r = self.line_states.len() - 1;
            let next = highlight_line(language, self.line_states[r], &self.buffer.line(r)).1;
            self.line_states.push(next);
            #[cfg(test)]
            {
                self.walked_lines += 1;
            }
        }

        let mut state = self.line_states[self.scroll_row];
        let mut rows = Vec::with_capacity(lines.len());
        for (i, text) in lines.iter().enumerate() {
            let r = self.scroll_row + i;
            if r >= self.buffer.line_count() {
                rows.push(vec![Style::Normal; text.chars().count()]);
                continue;
            }
            let (tokens, next) = highlight_line(language, state, &self.buffer.line(r));
            state = next;
            // Remember visible rows too, so scrolling down walks nothing.
            if self.line_states.len() == r + 1 {
                self.line_states.push(next);
            }
            rows.push(tokens.iter().skip(self.scroll_col).take(width).map(|t| token_style(*t)).collect());
        }
        rows
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

fn token_style(token: Token) -> Style {
    match token {
        Token::Plain => Style::Normal,
        Token::Keyword => Style::Keyword,
        Token::Type => Style::TypeName,
        Token::Str => Style::Str,
        Token::Comment => Style::Comment,
        Token::Number => Style::Number,
        Token::Meta => Style::Meta,
    }
}

/// `width` chars of `text` starting at char `skip`.
fn clip(text: &str, skip: usize, width: usize) -> String {
    text.chars().skip(skip).take(width).collect()
}

#[cfg(test)]
mod tests;
