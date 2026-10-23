//! Whole-line operations: delete, duplicate, move, comment toggle. Each acts
//! on the rows the selection touches (a selection that ends at column 0 of a
//! later row leaves that row out), else on the cursor's row, and is one undo
//! step.

use super::*;

impl Buffer {
    /// First and last row the operation acts on.
    fn block_rows(&self) -> (usize, usize) {
        match self.selection() {
            Some((from, to)) => (from.0, if to.1 == 0 && to.0 > from.0 { to.0 - 1 } else { to.0 }),
            None => (self.row, self.row),
        }
    }

    fn row_start(&self, row: usize) -> usize {
        self.rope.line_to_char(row)
    }

    /// Char index just past `row`'s line break (the end of the text for the last row).
    fn row_end(&self, row: usize) -> usize {
        if row + 1 < self.line_count() {
            self.rope.line_to_char(row + 1)
        } else {
            self.rope.len_chars()
        }
    }

    fn row_break(&self, row: usize) -> String {
        let line = self.rope.line(row);
        line.slice(line.len_chars() - break_len(line)..).to_string()
    }

    /// Deletes the block's rows with their line breaks; the cursor lands on
    /// the row that took their place, in the same column if it fits.
    pub fn delete_lines(&mut self) -> bool {
        let (r0, r1) = self.block_rows();
        let col = self.col;
        let mut from = self.row_start(r0);
        let to = self.row_end(r1);
        let last = r1 + 1 >= self.line_count();
        // The last row has no break of its own: take the one before it.
        if last && r0 > 0 {
            from -= break_len(self.rope.line(r0 - 1));
        }
        let (near, base) = if last { (r0.saturating_sub(1), self.row_start(r0.saturating_sub(1))) } else { (r1 + 1, from) };
        let after = if last && r0 == 0 { 0 } else { base + col.min(self.line_len(near)) };
        if from == to {
            return false;
        }
        self.replace_range_to(from, to, "", after);
        true
    }

    /// Copies the block's rows below themselves; the cursor and selection
    /// move onto the copy.
    pub fn duplicate_lines(&mut self) -> bool {
        let (r0, r1) = self.block_rows();
        let anchor = self.selection_anchor();
        let mut text = self.rope.slice(self.row_start(r0)..self.row_end(r1)).to_string();
        let end = self.row_end(r1);
        if !text.chars().last().is_some_and(is_break) {
            text.insert(0, '\n');
        }
        let shift = text.chars().count();
        let after = self.cursor_index() + shift;
        let rows = r1 - r0 + 1;
        self.replace_range_to(end, end, &text, after);
        self.restore_selection(anchor.map(|(r, c)| (r + rows, c)));
        true
    }

    /// Swaps the block with the row above it; the cursor and selection stay on it.
    pub fn move_lines_up(&mut self) -> bool {
        let (r0, r1) = self.block_rows();
        if r0 == 0 {
            return false;
        }
        self.swap_rows(r0 - 1, r1, true)
    }

    /// Swaps the block with the row below it.
    pub fn move_lines_down(&mut self) -> bool {
        let (r0, r1) = self.block_rows();
        if r1 + 1 >= self.line_count() {
            return false;
        }
        self.swap_rows(r0, r1 + 1, false)
    }

    /// Rewrites rows `a..=b`: the block and the one other row (the first when
    /// `up`, else the last) trade places. Each position keeps its own line break, so the last
    /// row of a file without a final break stays unbroken.
    fn swap_rows(&mut self, a: usize, b: usize, up: bool) -> bool {
        let anchor = self.selection_anchor();
        let contents: Vec<String> = (a..=b).map(|r| self.line(r)).collect();
        let breaks: Vec<String> = (a..=b).map(|r| self.row_break(r)).collect();
        let (block, other) = if up {
            (contents[1..].to_vec(), contents[0].clone())
        } else {
            (contents[..contents.len() - 1].to_vec(), contents[contents.len() - 1].clone())
        };
        let order: Vec<String> = if up { block.into_iter().chain([other.clone()]).collect() } else { [other.clone()].into_iter().chain(block).collect() };
        let text: String = order.iter().zip(&breaks).map(|(c, br)| format!("{c}{br}")).collect();
        let from = self.row_start(a);
        let to = self.row_end(b);
        let other_len = other.chars().count();
        // The other row passes the block, taking its own text and the first break with it.
        let delta = other_len + breaks[0].chars().count();
        let idx = self.cursor_index();
        let after = if up { idx - delta } else { idx + delta };
        self.replace_range_to(from, to, &text, after);
        let rows = |r: usize| if up { r - 1 } else { r + 1 };
        self.restore_selection(anchor.map(|(r, c)| (rows(r), c)));
        true
    }

    /// Comments the block's non-blank rows with `prefix` (at the shallowest
    /// indent), or removes it when every one of them already starts with it.
    /// False (no change) when every row is blank.
    pub fn toggle_comment(&mut self, prefix: &str) -> bool {
        let (r0, r1) = self.block_rows();
        let lines: Vec<String> = (r0..=r1).map(|r| self.line(r)).collect();
        let blank = |l: &String| l.trim().is_empty();
        if lines.iter().all(blank) {
            return false;
        }
        let indent = |l: &String| l.chars().take_while(|c| c.is_whitespace()).count();
        let remove = lines.iter().filter(|l| !blank(l)).all(|l| l.trim_start().starts_with(prefix));
        let min_indent = lines.iter().filter(|l| !blank(l)).map(indent).min().unwrap_or(0);
        let insert = format!("{prefix} ");
        let plen = prefix.chars().count();
        // (column the edit starts at, chars removed, chars added) per row.
        let edits: Vec<(usize, usize, usize)> = lines
            .iter()
            .map(|l| {
                if blank(l) {
                    (0, 0, 0)
                } else if remove {
                    let w = indent(l);
                    let space = usize::from(l.chars().nth(w + plen) == Some(' '));
                    (w, plen + space, 0)
                } else {
                    (min_indent, 0, insert.chars().count())
                }
            })
            .collect();
        let mut text = String::new();
        let mut new_starts = Vec::new();
        let mut at = self.row_start(r0);
        for (i, line) in lines.iter().enumerate() {
            let (col, del, add) = edits[i];
            let chars: Vec<char> = line.chars().collect();
            let mut new_line: String = chars[..col].iter().collect();
            if add > 0 {
                new_line.push_str(&insert);
            }
            new_line.extend(&chars[col + del..]);
            let row_break = self.row_break(r0 + i);
            new_starts.push(at);
            at += new_line.chars().count() + row_break.chars().count();
            text.push_str(&new_line);
            text.push_str(&row_break);
        }
        let total_shift = at as isize - self.row_end(r1) as isize;
        let new_index = |buf: &Buffer, (row, col): (usize, usize)| -> usize {
            if row < r0 {
                return buf.row_start(row) + col;
            }
            if row > r1 {
                return (buf.row_start(row) + col).saturating_add_signed(total_shift);
            }
            let (ecol, del, add) = edits[row - r0];
            let col = if col >= ecol + del { col - del + add } else if col > ecol { ecol } else { col };
            new_starts[row - r0] + col
        };
        let after = new_index(self, (self.row, self.col));
        let anchor = self.selection_anchor();
        let anchor_after = anchor.map(|p| new_index(self, p));
        let (from, to) = (self.row_start(r0), self.row_end(r1));
        self.replace_range_to(from, to, &text, after);
        if let Some(index) = anchor_after {
            let row = self.rope.char_to_line(index.min(self.rope.len_chars()));
            self.restore_selection(Some((row, index - self.row_start(row))));
        }
        true
    }
}
