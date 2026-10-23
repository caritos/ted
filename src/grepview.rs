//! "Find in files" view: a query box over the hits of `grep::search`. State,
//! key handling and rendering; no terminal code. The app runs the search
//! (`GrepAction::Search`) and hands the result back with `set_found`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::grep::{Found, Hit, MAX_HITS};
use crate::screen::{Row, Screen, Span, Style};
use crate::text::{clip, pad, window_start};

/// Rows PageUp / PageDown move.
const PAGE: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum GrepAction {
    None,
    Back,
    /// Run a search for this text and answer with `set_found`.
    Search(String),
    /// Open this file (relative to the app's location) at this 1-based line.
    Open { path: String, line: usize },
}

pub struct GrepView {
    query: String,
    found: Option<Found>,
    /// The query `found` belongs to.
    searched: String,
    selected: usize,
    message: String,
}

impl GrepView {
    pub fn new(query: &str) -> Self {
        GrepView { query: query.to_string(), found: None, searched: String::new(), selected: 0, message: String::new() }
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// The results for the query that was last handed out as a `Search`.
    pub fn set_found(&mut self, found: Found) {
        self.found = Some(found);
        self.searched = self.query.clone();
        self.selected = 0;
    }

    fn hits(&self) -> &[Hit] {
        self.found.as_ref().map_or(&[], |f| f.hits.as_slice())
    }

    /// The results are not for the text now in the box.
    fn stale(&self) -> bool {
        self.found.is_none() || self.searched != self.query
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> GrepAction {
        self.message.clear();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let last = self.hits().len().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::F(4) => return GrepAction::Back,
            KeyCode::Enter => {
                if self.stale() {
                    if !self.query.is_empty() {
                        return GrepAction::Search(self.query.clone());
                    }
                } else if let Some(hit) = self.hits().get(self.selected) {
                    return GrepAction::Open { path: hit.path.clone(), line: hit.line };
                }
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(PAGE),
            KeyCode::PageDown => self.selected = (self.selected + PAGE).min(last),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c) if !ctrl && !alt => self.query.push(c),
            _ => {}
        }
        GrepAction::None
    }

    /// Text arriving in one piece (a paste): line breaks never get in.
    pub fn paste(&mut self, text: &str) {
        self.query.extend(text.chars().filter(|c| !c.is_control()));
    }

    /// A body of `height - 2` rows, then the status and the query row.
    pub fn render(&self, width: usize, height: usize) -> Screen {
        let body_h = height.saturating_sub(2);
        let hits = self.hits();
        let selected = self.selected.min(hits.len().saturating_sub(1));
        let first = window_start(selected, body_h);
        let mut rows: Vec<Row> = hits
            .iter()
            .enumerate()
            .skip(first)
            .take(body_h)
            .map(|(n, hit)| {
                let text = format!("  {}:{}: {}", hit.path, hit.line, hit.text);
                if n == selected {
                    vec![Span { text: pad(&text, width), style: Style::Selected }]
                } else {
                    vec![Span { text: clip(&text, width), style: Style::Normal }]
                }
            })
            .collect();
        if hits.is_empty() && self.found.is_some() && body_h > 0 {
            rows.push(vec![Span { text: clip("  No matches", width), style: Style::Dim }]);
        }
        rows.resize_with(body_h, Vec::new);
        let (status, message) = if height >= 2 {
            let line = if self.message.is_empty() {
                // The end of a long query is where the typing happens.
                let line = format!("Find in files: {}_", self.query);
                let chars = line.chars().count();
                if chars > width && width > 0 {
                    format!("\u{2026}{}", line.chars().skip(chars - (width - 1)).collect::<String>())
                } else {
                    line
                }
            } else {
                self.message.clone()
            };
            (pad(&self.status(), width), clip(&line, width))
        } else {
            (String::new(), String::new())
        };
        Screen { rows, status, message, cursor: None }
    }

    fn status(&self) -> String {
        let Some(found) = &self.found else {
            return "Find in files: Enter searches".to_string();
        };
        let n = found.hits.len();
        let f = found.files_with_hits;
        let hits = if n == 1 { "hit" } else { "hits" };
        let files = if f == 1 { "file" } else { "files" };
        let cap = if found.capped { format!(" (first {MAX_HITS})") } else { String::new() };
        let old = if self.stale() { " - Enter searches again" } else { "" };
        format!("Find in files: {n} {hits} in {f} {files}{cap}{old}")
    }
}

#[cfg(test)]
mod tests;
