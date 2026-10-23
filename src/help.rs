//! The help screen: a scrollable list of every key. No terminal code.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::screen::{Row, Screen, Span, Style};
use crate::text::{clip, pad};

#[derive(Debug, PartialEq, Eq)]
pub enum HelpAction {
    None,
    Back,
    ToggleGit,
    ToggleTree,
}

/// Section title, then (keys, description) rows. Keys are separated by " / ".
pub(crate) const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Editor",
        &[
            ("Left / Right / Up / Down", "Move the cursor"),
            ("Home / End", "Start / end of the line"),
            ("PageUp / PageDown", "Move one screenful up / down"),
            ("Printable keys", "Insert text"),
            ("Enter", "New line"),
            ("Tab", "Insert 4 spaces"),
            ("Backspace / Delete", "Delete before / under the cursor"),
            ("Ctrl-S", "Save the current buffer"),
            ("Ctrl-Z / Ctrl-Y", "Undo / redo"),
            ("Ctrl-F", "Find text (matches are underlined)"),
            ("Ctrl-R", "Replace all (in the selection, if any)"),
            ("Ctrl-X", "Command prompt (Tab completes)"),
            ("F3", "Find the next match"),
            ("Ctrl-L", "Go to a line number"),
            ("F2", "Show / hide line numbers"),
            ("F1", "Show this help"),
        ],
    ),
    // Right after Editor so Ctrl-Q is on the first screen of a 24-row terminal.
    ("Quit", &[("Ctrl-Q", "Quit; with unsaved changes press it again")]),
    (
        "Selection and clipboard",
        &[
            ("Shift + move keys", "Select (arrows, Home / End, PageUp / PageDown)"),
            ("Ctrl-A", "Select everything"),
            ("Ctrl-C / Ctrl-K", "Copy / cut the selection"),
            ("Ctrl-V", "Paste (a terminal paste is one undo step)"),
            ("Esc", "Clear the selection; typing replaces it"),
        ],
    ),
    (
        "Words",
        &[
            ("Alt-Left / Alt-Right", "Move by word (Ctrl too; Shift selects)"),
            ("Alt-B / Alt-F", "Same (Option+arrows in macOS Terminal)"),
            ("Alt-Backspace / Alt-D", "Delete the previous / next word"),
        ],
    ),
    (
        "Find / go to line prompt",
        &[
            ("Enter", "Accept and stay at the match"),
            ("Esc", "Cancel; the cursor goes back"),
            ("Down / Ctrl-F", "Next match"),
            ("Up", "Previous match"),
        ],
    ),
    (
        "Replace prompt",
        &[
            ("Enter", "Replace: term, then With: text; Enter again replaces"),
            ("qreplace", "Command: same, but ask for each match"),
            ("y / n / a / q", "Ask mode: replace / skip / replace rest / stop"),
            ("Esc", "Cancel; nothing changes"),
        ],
    ),
    (
        "Help",
        &[
            ("Up / Down", "Scroll one line"),
            ("PageUp / PageDown", "Scroll one page"),
            ("Home / End", "Top / bottom"),
            ("q / Esc / F1", "Close help"),
        ],
    ),
    (
        "Files and buffers",
        &[
            ("Ctrl-G", "Changes view (again: back)"),
            ("Ctrl-T", "File tree (again: back)"),
            ("Ctrl-E", "Open a file by name (fuzzy finder)"),
            ("F4", "Find text in all files (Enter searches, then opens)"),
            ("Ctrl-O", "Buffer list"),
            ("Ctrl-N / Ctrl-P", "Next / previous buffer"),
            ("Ctrl-W", "Close the buffer (editor only; twice if unsaved)"),
        ],
    ),
    (
        "Views",
        &[
            ("Up / Down", "Changes, file tree, buffers: select"),
            ("PageUp / PageDown", "Scroll the diff (changes) / move 10 rows (tree)"),
            ("Home / End", "First / last row (tree, buffer list)"),
            ("Enter", "Open a file, toggle a directory, switch buffer"),
            ("Right / Left", "File tree: expand / collapse"),
            ("r", "Changes view: reset the baseline"),
            ("b", "Pick a baseline commit (Enter picks, Esc closes)"),
            ("q / Esc", "Back to the editor"),
            ("File finder", "Type to filter, Enter opens, Esc back"),
            ("Find in files", "Type, Enter searches, Enter opens a hit, Esc back"),
            ("?", "Show this help"),
        ],
    ),
];

/// Every help line: (text, style), with a blank line between sections.
fn lines() -> Vec<(String, Style)> {
    let key_w = HELP.iter().flat_map(|(_, rows)| rows.iter().map(|(k, _)| k.chars().count())).max().unwrap_or(0);
    let mut out = Vec::new();
    for (i, (title, rows)) in HELP.iter().enumerate() {
        if i > 0 {
            out.push((String::new(), Style::Normal));
        }
        out.push((title.to_string(), Style::Dir));
        for (keys, desc) in rows.iter() {
            out.push((format!("  {keys:<key_w$}  {desc}"), Style::Normal));
        }
    }
    out
}

#[derive(Debug, Default)]
pub struct Help {
    scroll: usize,
    /// Body height at the last render, for PageUp/PageDown.
    page: usize,
    message: String,
}

impl Help {
    pub fn new() -> Self {
        Help::default()
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn reset(&mut self) {
        self.scroll = 0;
        self.message.clear();
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> HelpAction {
        self.message.clear();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let total = lines().len();
        match key.code {
            KeyCode::Char('g') if ctrl => return HelpAction::ToggleGit,
            KeyCode::Char('t') if ctrl => return HelpAction::ToggleTree,
            _ if !plain => {}
            KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Esc | KeyCode::F(1) => return HelpAction::Back,
            // `render` clamps to the real maximum for the window it draws.
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll = (self.scroll + 1).min(total),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.page.max(1)),
            KeyCode::PageDown => self.scroll = (self.scroll + self.page.max(1)).min(total),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = total,
            _ => {}
        }
        HelpAction::None
    }

    /// A body of `height - 2` rows, then status and message. Clamps the scroll
    /// so the last line can sit at the bottom but nothing blank scrolls in.
    pub fn render(&mut self, width: usize, height: usize) -> Screen {
        let body_h = height.saturating_sub(2);
        let all = lines();
        self.page = body_h;
        self.scroll = self.scroll.min(all.len().saturating_sub(body_h));
        let mut rows: Vec<Row> = all
            .iter()
            .skip(self.scroll)
            .take(body_h)
            .map(|(text, style)| vec![Span { text: clip(text, width), style: *style }])
            .collect();
        rows.resize_with(body_h, Vec::new);
        let (status, message) = if height >= 2 {
            (pad(" Help  (q to close)", width), clip(&self.message, width))
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

    fn text(screen: &Screen, i: usize) -> String {
        screen.rows[i].iter().map(|s| s.text.as_str()).collect()
    }

    fn all_text(screen: &Screen) -> String {
        (0..screen.rows.len()).map(|i| text(screen, i)).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn table_lists_every_documented_key() {
        let keys: Vec<&str> = HELP.iter().flat_map(|(_, rows)| rows.iter().flat_map(|(k, _)| k.split(" / "))).collect();
        for name in [
            "Ctrl-S", "Ctrl-Q", "Ctrl-F", "Ctrl-R", "Ctrl-L", "Ctrl-X", "Ctrl-Z", "Ctrl-Y", "F3", "F4", "Ctrl-G", "Ctrl-T", "Ctrl-O", "Ctrl-N", "Ctrl-P", "Ctrl-W", "Home", "End", "PageUp", "PageDown", "Tab",
            "Backspace", "Delete", "Enter", "F1", "F2", "Up", "Down", "Left", "Right", "Esc", "r", "b", "q",
            "Ctrl-A", "Ctrl-C", "Ctrl-K", "Ctrl-V", "Shift + move keys",
        ] {
            assert!(keys.contains(&name), "help table lacks {name}");
        }
        for title in ["Editor", "Help", "Files and buffers", "Views", "Quit", "Selection and clipboard"] {
            assert!(HELP.iter().any(|(s, _)| *s == title), "no section {title}");
        }
    }

    #[test]
    fn render_shows_section_titles_and_key_rows_and_status() {
        let mut h = Help::new();
        let screen = h.render(80, 40);
        assert_eq!(screen.rows.len(), 38);
        let all = all_text(&screen);
        assert!(all.contains("Editor"));
        assert!(all.contains("Ctrl-S"));
        assert!(all.contains("Save"));
        assert!(screen.status.starts_with(" Help  (q to close)"), "{:?}", screen.status);
        assert_eq!(screen.cursor, None);
        assert_eq!(screen.status.chars().count(), 80);
    }

    #[test]
    fn scroll_clamps_at_both_ends() {
        let mut h = Help::new();
        let first = h.render(60, 6).rows.iter().map(|r| r[0].text.clone()).collect::<Vec<_>>();
        h.handle_key(key(KeyCode::Up));
        assert_eq!(h.render(60, 6).rows.iter().map(|r| r[0].text.clone()).collect::<Vec<_>>(), first);
        for _ in 0..500 {
            h.handle_key(key(KeyCode::Down));
        }
        let last = h.render(60, 6);
        let again = {
            h.handle_key(key(KeyCode::Down));
            h.render(60, 6)
        };
        assert_eq!(last, again);
        assert!(h.scroll() > 0);
        // The last table line is the last row, and one Up moves by one line.
        assert!(!text(&last, 3).trim().is_empty());
        h.handle_key(key(KeyCode::Up));
        assert_eq!(h.render(60, 6).rows[1], last.rows[0]);
        h.handle_key(key(KeyCode::Home));
        assert_eq!(h.render(60, 6).rows[0][0].text, first[0]);
        h.handle_key(key(KeyCode::End));
        assert_eq!(h.render(60, 6), last);
    }

    #[test]
    fn save_and_quit_are_on_the_first_screen_of_a_24_row_terminal() {
        let all = all_text(&Help::new().render(80, 24));
        assert!(all.contains("Ctrl-S") && all.contains("Ctrl-Q"), "{all}");
    }

    #[test]
    fn page_keys_move_a_page() {
        let mut h = Help::new();
        h.render(60, 10);
        h.handle_key(key(KeyCode::PageDown));
        assert_eq!(h.scroll(), 8);
        h.render(60, 10);
        h.handle_key(key(KeyCode::PageUp));
        assert_eq!(h.scroll(), 0);
    }

    #[test]
    fn tiny_sizes_never_panic() {
        for w in 0..=6 {
            for h in 0..=6 {
                let mut help = Help::new();
                for k in [KeyCode::Down, KeyCode::PageDown, KeyCode::End, KeyCode::Up] {
                    let screen = help.render(w, h);
                    assert_eq!(screen.rows.len(), h.saturating_sub(2));
                    for r in &screen.rows {
                        let n: usize = r.iter().map(|s| s.text.chars().count()).sum();
                        assert!(n <= w);
                    }
                    assert!(screen.status.chars().count() <= w && screen.message.chars().count() <= w);
                    help.handle_key(key(k));
                }
            }
        }
    }

    #[test]
    fn long_lines_are_clipped_not_wrapped() {
        let screen = Help::new().render(12, 30);
        for r in &screen.rows {
            let n: usize = r.iter().map(|s| s.text.chars().count()).sum();
            assert!(n <= 12);
        }
    }

    #[test]
    fn keys_q_esc_f1_go_back_and_views_toggle() {
        let mut h = Help::new();
        assert_eq!(h.handle_key(key(KeyCode::Char('q'))), HelpAction::Back);
        assert_eq!(h.handle_key(key(KeyCode::Esc)), HelpAction::Back);
        assert_eq!(h.handle_key(key(KeyCode::F(1))), HelpAction::Back);
        assert_eq!(h.handle_key(key(KeyCode::Char('?'))), HelpAction::Back);
        assert_eq!(h.handle_key(ctrl('g')), HelpAction::ToggleGit);
        assert_eq!(h.handle_key(ctrl('t')), HelpAction::ToggleTree);
        assert_eq!(h.handle_key(key(KeyCode::Char('x'))), HelpAction::None);
    }

    #[test]
    fn message_shows_until_next_key() {
        let mut h = Help::new();
        h.set_message("hi".into());
        assert_eq!(h.render(20, 6).message, "hi");
        h.handle_key(key(KeyCode::Down));
        assert_eq!(h.render(20, 6).message, "");
    }

    #[test]
    fn no_help_row_is_wider_than_80_columns() {
        for (text, _) in lines() {
            assert!(text.chars().count() <= 80, "{text:?}");
        }
    }

    #[test]
    fn table_text_has_no_control_chars() {
        let screen = Help::new().render(80, 60);
        assert!(!all_text(&screen).chars().any(|c| c.is_control() && c != '\n'));
    }
}
