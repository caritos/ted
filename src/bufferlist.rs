//! The list of open buffers: selection state, key handling and rendering.
//! No terminal code; the app supplies the entries.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::screen::{Row, Screen, Span, Style};
use crate::text::{clip, pad, window_start};

#[derive(Debug, PartialEq, Eq)]
pub enum ListAction {
    None,
    Back,
    /// Switch to the buffer at this index and return to the editor.
    Switch(usize),
    ToggleGit,
    ToggleTree,
}

#[derive(Debug, Default)]
pub struct BufferList {
    selected: usize,
    message: String,
}

impl BufferList {
    pub fn new(selected: usize) -> Self {
        BufferList { selected, message: String::new() }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Selects `index` and drops any message; used on each visit.
    pub fn reset(&mut self, index: usize) {
        self.selected = index;
        self.message.clear();
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// `len` is the number of buffers, so the selection can be clamped.
    pub fn handle_key(&mut self, key: KeyEvent, len: usize) -> ListAction {
        self.message.clear();
        let last = len.saturating_sub(1);
        self.selected = self.selected.min(last);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('g') if ctrl => return ListAction::ToggleGit,
            KeyCode::Char('t') if ctrl => return ListAction::ToggleTree,
            KeyCode::Char('o') if ctrl => return ListAction::Back,
            KeyCode::Char('q') if plain => return ListAction::Back,
            KeyCode::Esc => return ListAction::Back,
            _ if !plain => {}
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            KeyCode::Enter => return ListAction::Switch(self.selected),
            _ => {}
        }
        ListAction::None
    }

    /// `entries` are (file name or `[No Name]`, modified) per buffer; `current`
    /// is marked with `>`. A body of `height - 2` rows, then status and message.
    pub fn render(&self, entries: &[(String, bool)], current: usize, width: usize, height: usize) -> Screen {
        let body_h = height.saturating_sub(2);
        let selected = self.selected.min(entries.len().saturating_sub(1));
        let first = window_start(selected, body_h);
        let mut rows: Vec<Row> = entries
            .iter()
            .enumerate()
            .skip(first)
            .take(body_h)
            .map(|(i, (name, modified))| {
                let mark = if i == current { '>' } else { ' ' };
                let tag = if *modified { " [+]" } else { "" };
                let text = format!("{mark}{:>2}  {name}{tag}", i + 1);
                if i == selected {
                    vec![Span { text: pad(&text, width), style: Style::Selected }]
                } else {
                    vec![Span { text: clip(&text, width), style: Style::Normal }]
                }
            })
            .collect();
        rows.resize_with(body_h, Vec::new);
        let (status, message) = if height >= 2 {
            let n = entries.len();
            let noun = if n == 1 { "buffer" } else { "buffers" };
            (pad(&format!("Buffers: {n} {noun}"), width), clip(&self.message, width))
        } else {
            (String::new(), String::new())
        };
        Screen { rows, status, message, cursor: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn entries() -> Vec<(String, bool)> {
        vec![("a.txt".into(), false), ("b.txt".into(), true), ("[No Name]".into(), false)]
    }

    fn text(screen: &Screen, i: usize) -> String {
        screen.rows[i].iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn renders_numbers_current_marker_and_modified_tag() {
        let screen = BufferList::new(0).render(&entries(), 1, 30, 8);
        assert_eq!(text(&screen, 0).trim_end(), "  1  a.txt");
        assert_eq!(text(&screen, 1).trim_end(), "> 2  b.txt [+]");
        assert_eq!(text(&screen, 2).trim_end(), "  3  [No Name]");
        assert_eq!(screen.rows.len(), 6);
        assert!(screen.status.starts_with("Buffers: 3 buffers"));
        assert_eq!(screen.cursor, None);
    }

    #[test]
    fn selected_row_has_selected_style() {
        let screen = BufferList::new(2).render(&entries(), 0, 30, 8);
        assert_eq!(screen.rows[2][0].style, Style::Selected);
        assert_eq!(screen.rows[0][0].style, Style::Normal);
    }

    #[test]
    fn hostile_names_are_sanitized() {
        let list = vec![("a\x1b[31m\n.txt".to_string(), true)];
        let screen = BufferList::new(0).render(&list, 0, 40, 4);
        let row = text(&screen, 0);
        assert!(row.contains("a?[31m?.txt [+]"), "{row:?}");
        assert!(!row.chars().any(|c| c.is_control()));
    }

    #[test]
    fn long_selection_scrolls_into_view() {
        let many: Vec<(String, bool)> = (0..20).map(|i| (format!("f{i}"), false)).collect();
        let mut l = BufferList::new(0);
        l.handle_key(key(KeyCode::End), 20);
        let screen = l.render(&many, 0, 20, 5);
        assert_eq!(screen.rows[2][0].style, Style::Selected);
        assert!(text(&screen, 2).contains("f19"));
    }

    #[test]
    fn tiny_sizes_never_panic() {
        for w in 0..=6 {
            for h in 0..=6 {
                for list in [Vec::new(), entries()] {
                    let screen = BufferList::new(5).render(&list, 9, w, h);
                    assert_eq!(screen.rows.len(), h.saturating_sub(2));
                    for r in &screen.rows {
                        let n: usize = r.iter().map(|s| s.text.chars().count()).sum();
                        assert!(n <= w);
                    }
                }
            }
        }
    }

    #[test]
    fn up_down_clamp_and_home_end() {
        let mut l = BufferList::new(0);
        l.handle_key(key(KeyCode::Up), 3);
        assert_eq!(l.selected(), 0);
        l.handle_key(key(KeyCode::Down), 3);
        l.handle_key(key(KeyCode::Down), 3);
        l.handle_key(key(KeyCode::Down), 3);
        assert_eq!(l.selected(), 2);
        l.handle_key(key(KeyCode::Home), 3);
        assert_eq!(l.selected(), 0);
        l.handle_key(key(KeyCode::End), 3);
        assert_eq!(l.selected(), 2);
    }

    #[test]
    fn enter_switches_to_selected() {
        let mut l = BufferList::new(1);
        assert_eq!(l.handle_key(key(KeyCode::Enter), 3), ListAction::Switch(1));
    }

    #[test]
    fn q_esc_ctrl_o_go_back_and_ctrl_g_t_toggle_views() {
        let mut l = BufferList::new(0);
        assert_eq!(l.handle_key(key(KeyCode::Char('q')), 3), ListAction::Back);
        assert_eq!(l.handle_key(key(KeyCode::Esc), 3), ListAction::Back);
        assert_eq!(l.handle_key(ctrl('o'), 3), ListAction::Back);
        assert_eq!(l.handle_key(ctrl('g'), 3), ListAction::ToggleGit);
        assert_eq!(l.handle_key(ctrl('t'), 3), ListAction::ToggleTree);
        assert_eq!(l.handle_key(key(KeyCode::Char('x')), 3), ListAction::None);
    }

    #[test]
    fn stale_selection_is_clamped_by_keys() {
        let mut l = BufferList::new(7);
        assert_eq!(l.handle_key(key(KeyCode::Enter), 2), ListAction::Switch(1));
        assert_eq!(BufferList::new(3).handle_key(key(KeyCode::Enter), 0), ListAction::Switch(0));
    }

    #[test]
    fn message_shows_until_next_key() {
        let mut l = BufferList::new(0);
        l.set_message("hi".into());
        assert_eq!(l.render(&entries(), 0, 20, 6).message, "hi");
        l.handle_key(key(KeyCode::Down), 3);
        assert_eq!(l.render(&entries(), 0, 20, 6).message, "");
    }
}
