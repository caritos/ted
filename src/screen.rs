//! A styled frame, shared by the editor and the git view. `main.rs` draws it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Selected,
    /// A search match while the Find prompt is open (underlined). It replaces
    /// the syntax colour on those cells; there is no combined style.
    Match,
    Added,
    Removed,
    Hunk,
    Dim,
    Dir,
    Keyword,
    TypeName,
    Str,
    Comment,
    Number,
    Meta,
    LineNo,
    /// The gutter number of the cursor's row: bold instead of dim.
    CurrentLineNo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

pub type Row = Vec<Span>;

/// Everything the terminal layer needs to draw one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// Exactly the visible body rows, already clipped to the width.
    pub rows: Vec<Row>,
    pub status: String,
    pub message: String,
    /// Cursor position as (row, column) within `rows`; `None` hides it.
    pub cursor: Option<(usize, usize)>,
}
