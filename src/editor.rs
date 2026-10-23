use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::buffer::{Buffer, Found};
use crate::command;
use crate::highlight::{highlight_line, Language, LineState, Token};
use crate::screen::Style;
use crate::text::{display_col, layout, sanitize_char, TAB_STOP};

/// Longest text a prompt takes from a paste (chars).
const MAX_PROMPT_TEXT: usize = 4096;

/// Chars that end a line (the ones ropey also treats as breaks).
fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Page size before the first frame tells the editor the screen height.
const DEFAULT_PAGE: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// Everything the terminal layer needs to draw one frame. All text is
/// sanitized (see `clip`): it holds no control characters.
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    Find,
    Goto,
    Command,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FindStatus {
    Normal,
    Wrapped,
    NotFound,
}

struct Prompt {
    kind: PromptKind,
    text: String,
    /// Cursor and scroll (row, col) when the prompt opened; Esc restores them.
    origin: (usize, usize),
    scroll: (usize, usize),
    /// Where the selection started when the prompt opened; Esc brings the
    /// selection back (an accepted Find / Go to has cleared it).
    anchor: Option<(usize, usize)>,
    /// Current match, the start for next/previous.
    current: Option<(usize, usize)>,
    status: FindStatus,
}

pub struct Editor {
    buffer: Buffer,
    message: String,
    /// Extra status text after the name and `[+]` (e.g. `[2/3]`); empty is none.
    tag: String,
    scroll_row: usize,
    scroll_col: usize,
    /// Lines PageUp/PageDown move: one screenful less one, from the last frame.
    page: usize,
    /// Line number gutter on or off (hidden anyway on a narrow terminal).
    line_numbers: bool,
    /// Open Find / Go to line prompt; it gets every key.
    prompt: Option<Prompt>,
    /// Text of an accepted command prompt, until `take_command`.
    command: Option<String>,
    /// Last accepted search term, for Ctrl-F prefill and F3.
    last_search: String,
    /// Term accepted with Enter, until `take_search`.
    accepted_search: Option<String>,
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
            tag: String::new(),
            scroll_row: 0,
            scroll_col: 0,
            page: DEFAULT_PAGE,
            line_numbers: false,
            prompt: None,
            command: None,
            last_search: String::new(),
            accepted_search: None,
            line_states: Vec::new(),
            highlighted: None,
            #[cfg(test)]
            walked_lines: 0,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn set_tag(&mut self, tag: String) {
        self.tag = tag;
    }

    pub fn set_line_numbers(&mut self, on: bool) {
        self.line_numbers = on;
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Drops the selection (the app does this when the buffer is left or shown).
    pub fn clear_selection(&mut self) {
        self.buffer.clear_selection();
    }

    /// The selected text, if any (copy).
    pub fn selected_text(&self) -> Option<String> {
        self.buffer.selected_text()
    }

    /// Deletes the selection as one undo step and returns its text (cut);
    /// `None` and no change when nothing is selected.
    pub fn cut_selection(&mut self) -> Option<String> {
        let text = self.buffer.selected_text()?;
        self.buffer.delete_selection();
        self.message.clear();
        Some(text)
    }

    /// Inserts `text` at the cursor, replacing the selection, as one undo
    /// step (paste). The caller has normalised the line breaks.
    pub fn insert_text(&mut self, text: &str) {
        self.message.clear();
        self.buffer.insert_str(text);
    }

    pub fn select_all(&mut self) {
        self.buffer.select_all();
    }

    /// Text arriving in one piece (bracketed paste) while a prompt is open:
    /// line breaks never get in, so a paste cannot accept the prompt; a Go to
    /// prompt keeps only digits. Returns false when no prompt is open.
    pub fn paste_into_prompt(&mut self, text: &str) -> bool {
        let Some(mut prompt) = self.prompt.take() else {
            return false;
        };
        let room = MAX_PROMPT_TEXT.saturating_sub(prompt.text.chars().count());
        let added: String = match prompt.kind {
            PromptKind::Goto => text.chars().filter(char::is_ascii_digit).take(room).collect(),
            _ => text.chars().filter(|&c| !is_line_break(c)).take(room).collect(),
        };
        if !added.is_empty() {
            prompt.text.push_str(&added);
            if prompt.kind == PromptKind::Find {
                self.search_from_origin(&mut prompt);
            }
        }
        self.prompt = Some(prompt);
        true
    }

    pub fn in_prompt(&self) -> bool {
        self.prompt.is_some()
    }

    /// Closes an open prompt as Esc does: cursor and scroll go back.
    pub fn cancel_prompt(&mut self) {
        if let Some(prompt) = self.prompt.take() {
            self.cancel(&prompt);
        }
    }

    /// Esc: cursor, scroll and selection go back to how they were.
    fn cancel(&mut self, prompt: &Prompt) {
        self.restore_origin(prompt);
        self.buffer.restore_selection(prompt.anchor);
    }

    fn restore_origin(&mut self, prompt: &Prompt) {
        self.buffer.set_cursor(prompt.origin.0, prompt.origin.1);
        self.scroll_row = prompt.scroll.0;
        self.scroll_col = prompt.scroll.1;
    }

    fn open_prompt(&mut self, kind: PromptKind) {
        if kind == PromptKind::Command {
            self.command = None;
        }
        let mut prompt = Prompt {
            kind,
            text: String::new(),
            origin: self.buffer.cursor(),
            scroll: (self.scroll_row, self.scroll_col),
            anchor: self.buffer.selection_anchor(),
            current: None,
            status: FindStatus::Normal,
        };
        if kind == PromptKind::Find && !self.last_search.is_empty() {
            prompt.text = self.last_search.clone();
            self.search_from_origin(&mut prompt);
        }
        self.prompt = Some(prompt);
    }

    /// Incremental search: always from where the prompt opened.
    fn search_from_origin(&mut self, prompt: &mut Prompt) {
        if prompt.text.is_empty() {
            self.restore_origin(prompt);
            prompt.current = None;
            prompt.status = FindStatus::Normal;
            return;
        }
        let found = self.buffer.find(&prompt.text, prompt.origin, true, true);
        self.apply_found(prompt, found);
    }

    fn step(&mut self, prompt: &mut Prompt, forward: bool) {
        if prompt.text.is_empty() {
            return;
        }
        let from = prompt.current.unwrap_or(prompt.origin);
        let found = self.buffer.find(&prompt.text, from, forward, false);
        self.apply_found(prompt, found);
    }

    fn apply_found(&mut self, prompt: &mut Prompt, found: Option<Found>) {
        match found {
            Some(f) => {
                self.buffer.set_cursor(f.row, f.col);
                prompt.current = Some(self.buffer.cursor());
                prompt.status = if f.wrapped { FindStatus::Wrapped } else { FindStatus::Normal };
            }
            None => prompt.status = FindStatus::NotFound,
        }
    }

    fn prompt_key(&mut self, mut prompt: Prompt, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let find = prompt.kind == PromptKind::Find;
        match key.code {
            KeyCode::Esc => {
                self.cancel(&prompt);
                return;
            }
            KeyCode::Enter => {
                // Accepting a search or a jump that moves on clears the
                // selection; one that moves nowhere acts like Esc and keeps
                // it; a command may still act on it.
                match prompt.kind {
                    PromptKind::Find => {
                        if prompt.status == FindStatus::NotFound || prompt.text.is_empty() {
                            self.cancel(&prompt);
                        } else {
                            self.buffer.clear_selection();
                        }
                        if !prompt.text.is_empty() {
                            self.accepted_search = Some(prompt.text.clone());
                            self.last_search = prompt.text;
                        }
                    }
                    PromptKind::Goto => {
                        let n = prompt.text.parse::<usize>().unwrap_or(usize::MAX);
                        if !prompt.text.is_empty() && n > 0 {
                            self.buffer.go_to_line(n);
                        } else {
                            self.cancel(&prompt);
                        }
                    }
                    PromptKind::Command => {
                        if !prompt.text.trim().is_empty() {
                            self.command = Some(prompt.text);
                        }
                    }
                }
                return;
            }
            KeyCode::Backspace => {
                if prompt.text.pop().is_some() && find {
                    self.search_from_origin(&mut prompt);
                }
            }
            KeyCode::Char(c) if !ctrl && !alt => match prompt.kind {
                PromptKind::Find => {
                    // A longer query cannot match where its prefix did not
                    // (also across the smart-case switch), so skip the scan.
                    let hopeless = prompt.status == FindStatus::NotFound;
                    prompt.text.push(c);
                    if !hopeless {
                        self.search_from_origin(&mut prompt);
                    }
                }
                PromptKind::Goto => {
                    if c.is_ascii_digit() {
                        prompt.text.push(c);
                    }
                }
                PromptKind::Command => prompt.text.push(c),
            },
            KeyCode::Tab if prompt.kind == PromptKind::Command => {
                let word = prompt.text.trim_start();
                if !word.contains(char::is_whitespace) {
                    let lead = prompt.text.len() - word.len();
                    let done = command::complete(word);
                    prompt.text.truncate(lead);
                    prompt.text.push_str(&done);
                }
            }
            KeyCode::Char('f') if ctrl && find => self.step(&mut prompt, true),
            KeyCode::Down if find => self.step(&mut prompt, true),
            KeyCode::Up if find => self.step(&mut prompt, false),
            _ => {}
        }
        self.prompt = Some(prompt);
    }

    /// The text of the command accepted with Enter, once; `None` if the
    /// prompt was cancelled or blank.
    pub fn take_command(&mut self) -> Option<String> {
        self.command.take()
    }

    /// Sets the remembered term (the app's shared one) without marking it new.
    pub fn set_last_search(&mut self, term: &str) {
        if self.last_search != term {
            self.last_search = term.to_string();
        }
    }

    /// The term accepted with Enter, once; `None` if the prompt was
    /// cancelled or empty.
    pub fn take_search(&mut self) -> Option<String> {
        self.accepted_search.take()
    }

    /// F3: next match of the remembered term after the cursor.
    fn find_again(&mut self) {
        if self.last_search.is_empty() {
            self.message = "No previous search".to_string();
            return;
        }
        match self.buffer.find(&self.last_search, self.buffer.cursor(), true, false) {
            Some(f) => self.buffer.set_cursor(f.row, f.col),
            None => self.message = format!("Not found: {}", self.last_search),
        }
    }

    fn prompt_line(&self, prompt: &Prompt) -> String {
        match prompt.kind {
            PromptKind::Goto => format!("Go to line: {}_", prompt.text),
            PromptKind::Command => {
                let mut line = format!("Command: {}_", prompt.text);
                let word = prompt.text.trim_start();
                if !word.contains(char::is_whitespace) {
                    let found = command::matches(word);
                    if !found.is_empty() {
                        line.push_str("  ");
                        line.push_str(&found.join(" "));
                    }
                }
                line
            }
            PromptKind::Find => match prompt.status {
                FindStatus::Normal => format!("Find: {}_", prompt.text),
                FindStatus::Wrapped => format!("Find: {} (wrapped)", prompt.text),
                FindStatus::NotFound => format!("Find: {} (not found)", prompt.text),
            },
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Flow {
        self.message.clear();
        if let Some(prompt) = self.prompt.take() {
            self.prompt_key(prompt, key);
            return Flow::Continue;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let select = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Char('f') if ctrl => self.open_prompt(PromptKind::Find),
            KeyCode::Char('l') if ctrl => self.open_prompt(PromptKind::Goto),
            KeyCode::Char('x') if ctrl => self.open_prompt(PromptKind::Command),
            KeyCode::Char('z') if ctrl => self.undo(),
            KeyCode::Char('y') if ctrl => self.redo(),
            KeyCode::F(3) => self.find_again(),
            KeyCode::Char('a') if ctrl => self.buffer.select_all(),
            KeyCode::Char(c) if !ctrl && !alt => self.buffer.insert_char(c),
            KeyCode::Enter => self.buffer.insert_newline(),
            KeyCode::Backspace => self.buffer.backspace(),
            KeyCode::Delete => self.buffer.delete(),
            KeyCode::Esc => self.buffer.clear_selection(),
            KeyCode::Tab => {
                if self.buffer.selection().is_some() {
                    // One undo step together with the deletion.
                    self.buffer.insert_str(&" ".repeat(TAB_STOP));
                } else {
                    for _ in 0..TAB_STOP {
                        self.buffer.insert_char(' ');
                    }
                }
            }
            KeyCode::Left => self.buffer.move_left_select(select),
            KeyCode::Right => self.buffer.move_right_select(select),
            KeyCode::Up => self.buffer.move_up_select(select),
            KeyCode::Down => self.buffer.move_down_select(select),
            KeyCode::Home => self.buffer.move_home_select(select),
            KeyCode::End => self.buffer.move_end_select(select),
            KeyCode::PageUp => self.buffer.move_up_by_select(self.page, select),
            KeyCode::PageDown => self.buffer.move_down_by_select(self.page, select),
            _ => {}
        }
        Flow::Continue
    }

    /// Lays out one frame for a `width` x `height` terminal: text rows, then a
    /// status bar, then a message row. Scrolls just enough to show the cursor.
    pub fn view(&mut self, width: usize, height: usize) -> View {
        let text_rows = height.saturating_sub(2);
        let (row, col) = self.buffer.cursor();
        self.page = text_rows.saturating_sub(1).max(1);

        if text_rows > 0 {
            if row < self.scroll_row {
                self.scroll_row = row;
            } else if row >= self.scroll_row + text_rows {
                self.scroll_row = row + 1 - text_rows;
            }
        }
        // The gutter takes columns from the left; scrolling uses the rest.
        let gutter = self.gutter_width(width);
        let text_width = width - gutter;
        // Columns here are display columns: a tab spans up to the next stop.
        let col = display_col(&self.buffer.line(row), col);
        if text_width > 0 {
            if col < self.scroll_col {
                self.scroll_col = col;
            } else if col >= self.scroll_col + text_width {
                self.scroll_col = col + 1 - text_width;
            }
        }

        let mut text: Vec<String> = Vec::with_capacity(text_rows);
        let mut layouts: Vec<Vec<usize>> = Vec::with_capacity(text_rows);
        for i in 0..text_rows {
            let r = self.scroll_row + i;
            if r < self.buffer.line_count() {
                let line = self.buffer.line(r);
                let cells = layout(&line, self.scroll_col, text_width);
                text.push(draw_cells(&line, &cells));
                layouts.push(cells);
            } else {
                text.push(clip("~", 0, text_width));
                layouts.push(Vec::new());
            }
        }
        let mut styles = self.styles(&text, &layouts);
        self.mark_selection(&mut styles, &layouts);
        let mut lines = text;
        if gutter > 0 {
            let digits = gutter - 1;
            for (i, (line, row_styles)) in lines.iter_mut().zip(styles.iter_mut()).enumerate() {
                let r = self.scroll_row + i;
                let number = if r < self.buffer.line_count() {
                    format!("{:>digits$} ", r + 1)
                } else {
                    " ".repeat(gutter)
                };
                line.insert_str(0, &number);
                row_styles.splice(0..0, std::iter::repeat_n(Style::LineNo, gutter));
            }
        }

        View {
            lines,
            styles,
            status: clip(&self.status_line(width), 0, width),
            message: match &self.prompt {
                Some(prompt) => clip(&self.prompt_line(prompt), 0, width),
                None => clip(&self.message, 0, width),
            },
            // With no text rows or no width the scroll is left alone, so the
            // cursor may sit above or left of it: clamp to 0, never underflow.
            cursor: (
                row.saturating_sub(self.scroll_row),
                gutter + col.saturating_sub(self.scroll_col),
            ),
        }
    }

    /// Draws the selection over `styles` (it wins over syntax colors). Each
    /// line's selected cells are styled; a line break itself is not drawn.
    fn mark_selection(&self, styles: &mut [Vec<Style>], layouts: &[Vec<usize>]) {
        let Some((start, end)) = self.buffer.selection() else {
            return;
        };
        for (i, (row_styles, cells)) in styles.iter_mut().zip(layouts).enumerate() {
            let r = self.scroll_row + i;
            if r < start.0 || r > end.0 {
                continue;
            }
            let from = if r == start.0 { start.1 } else { 0 };
            let to = if r == end.0 { end.1 } else { usize::MAX };
            for (style, &c) in row_styles.iter_mut().zip(cells) {
                if c >= from && c < to {
                    *style = Style::Selected;
                }
            }
        }
    }

    /// Gutter columns (number digits plus one space) for a `width` terminal:
    /// 0 when off or when fewer than 10 text columns would remain.
    fn gutter_width(&self, width: usize) -> usize {
        if !self.line_numbers {
            return 0;
        }
        let digits = self.buffer.line_count().to_string().len().max(3);
        let gutter = digits + 1;
        if width < gutter + 10 {
            0
        } else {
            gutter
        }
    }

    /// One style per displayed char of `lines` (the clipped visible rows).
    /// Multi-line state (block comments, strings) at the top of the viewport
    /// comes from `line_states`, which is cut back to the first edited row
    /// and then extended only as far as the first visible row.
    fn styles(&mut self, lines: &[String], layouts: &[Vec<usize>]) -> Vec<Vec<Style>> {
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
            rows.push(layouts[i].iter().map(|&c| token_style(tokens[c])).collect());
        }
        rows
    }

    /// Moves to column 0 of the 1-based `n`, clamped to the buffer.
    pub fn go_to_line(&mut self, n: usize) {
        self.buffer.go_to_line(n);
    }

    /// Undoes the last edit step; says so when there is nothing to undo.
    pub fn undo(&mut self) {
        if !self.buffer.undo() {
            self.message = "Nothing to undo".to_string();
        }
    }

    /// Redoes the last undone step; says so when there is nothing to redo.
    pub fn redo(&mut self) {
        if !self.buffer.redo() {
            self.message = "Nothing to redo".to_string();
        }
    }

    pub fn save(&mut self) {
        if self.buffer.path().is_none() {
            self.message = "No file name: start ted with a path to save.".to_string();
            return;
        }
        match self.buffer.save() {
            Ok(()) => self.message = format!("Saved {} lines", self.buffer.line_count()),
            Err(e) => self.message = format!("Save failed: {e}"),
        }
    }

    /// Saves under a new path; shows the result as the message.
    pub fn save_as(&mut self, path: std::path::PathBuf) {
        match self.buffer.save_as(path) {
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
        let tag = if self.tag.is_empty() { String::new() } else { format!(" {}", self.tag) };
        let left = format!(" {name}{marker}{tag}");
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

/// The cells of `line` chosen by `text::layout`, each the sanitized source
/// char (a tab cell is a space). Only the chars the cells need are read.
fn draw_cells(line: &str, cells: &[usize]) -> String {
    let (Some(&first), Some(&last)) = (cells.first(), cells.last()) else {
        return String::new();
    };
    let chars: Vec<char> = line.chars().skip(first).take(last - first + 1).collect();
    cells.iter().map(|&i| sanitize_char(chars[i - first])).collect()
}

/// `width` chars of `text` starting at char `skip`, sanitized one char for
/// one char. Every string in a `View` (rows, status, message) is built here,
/// so text from the file, its name or an error can never reach the terminal
/// as an escape sequence, while columns and styles stay aligned.
fn clip(text: &str, skip: usize, width: usize) -> String {
    text.chars().skip(skip).take(width).map(sanitize_char).collect()
}

#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod gutter_tests;
#[cfg(test)]
mod prompt_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod tab_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
