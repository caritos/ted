//! Syntax tokenizer: classifies every character of a line, carrying a small
//! state across lines. Pure and std-only; works on `char`s, never bytes.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
}

impl Language {
    /// By file extension (`rs`, case-sensitive); anything else is `None`.
    pub fn from_path(path: &Path) -> Option<Language> {
        match path.extension()?.to_str()? {
            "rs" => Some(Language::Rust),
            _ => None,
        }
    }
}

/// The line-comment prefix for the file type (by extension), if known.
pub fn comment_prefix(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" | "js" | "jsx" | "ts" | "tsx" | "c" | "h" | "cc" | "cpp" | "hpp" | "java" | "go" | "swift" | "kt" | "cs" | "php" | "scala" | "dart" => Some("//"),
        "py" | "sh" | "bash" | "zsh" | "rb" | "toml" | "yaml" | "yml" | "pl" | "r" | "mk" | "conf" => Some("#"),
        "sql" | "lua" | "hs" | "elm" => Some("--"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Plain,
    Keyword,
    Type,
    Str,
    Comment,
    Number,
    Meta,
}

/// What is still open at the start of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineState {
    #[default]
    Normal,
    /// Inside a (nested) block comment of this depth.
    BlockComment(u32),
    Str,
    /// Inside a raw string closed by a quote and this many `#`.
    RawStr(u32),
}

const KEYWORDS: [&str; 37] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
];

/// One `Token` per `char` of `line`, and the state at the start of the next
/// line. Never panics; linear in the length of the line.
pub fn highlight_line(language: Language, state: LineState, line: &str) -> (Vec<Token>, LineState) {
    match language {
        Language::Rust => rust_line(state, line),
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// End of a string body starting at `i` (just after the opening quote, or
/// the start of a continued line): index after the closing quote, and
/// whether it was found.
fn scan_str(chars: &[char], mut i: usize) -> (usize, bool) {
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '"' => return (i + 1, true),
            _ => i += 1,
        }
    }
    (chars.len(), false)
}

/// End of a raw string body: a quote followed by `hashes` `#`.
fn scan_raw(chars: &[char], mut i: usize, hashes: usize) -> (usize, bool) {
    while i < chars.len() {
        if chars[i] == '"' {
            let run = chars[i + 1..].iter().take(hashes).take_while(|&&c| c == '#').count();
            if run == hashes {
                return (i + 1 + hashes, true);
            }
        }
        i += 1;
    }
    (chars.len(), false)
}

/// Scan a block comment body from `i` at `depth`; returns the index after
/// the scan and the depth still open (0 when closed).
fn scan_block(chars: &[char], mut i: usize, mut depth: u32) -> (usize, u32) {
    while i < chars.len() {
        let next = chars.get(i + 1).copied();
        match (chars[i], next) {
            ('/', Some('*')) => {
                depth = depth.saturating_add(1);
                i += 2;
            }
            ('*', Some('/')) => {
                depth = depth.saturating_sub(1);
                i += 2;
                if depth == 0 {
                    return (i, 0);
                }
            }
            _ => i += 1,
        }
    }
    (chars.len(), depth)
}

/// If the `'` at `q` starts a char literal, the index after it.
fn char_literal_end(chars: &[char], q: usize) -> Option<usize> {
    match chars.get(q + 1) {
        Some('\\') => {
            let mut j = q + 3;
            while j < chars.len() {
                if chars[j] == '\'' {
                    return Some(j + 1);
                }
                j += 1;
            }
            Some(chars.len())
        }
        Some(_) if chars.get(q + 2) == Some(&'\'') => Some(q + 3),
        _ => None,
    }
}

fn classify_word(word: &[char]) -> Token {
    let text: String = word.iter().collect();
    if KEYWORDS.contains(&text.as_str()) {
        Token::Keyword
    } else if text == "Self"
        || (word[0].is_uppercase() && word.iter().any(|c| c.is_lowercase()))
    {
        Token::Type
    } else {
        Token::Plain
    }
}

fn rust_line(state: LineState, line: &str) -> (Vec<Token>, LineState) {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut out = vec![Token::Plain; n];
    let mut i = 0;

    // Finish whatever the previous line left open.
    match state {
        LineState::Normal => {}
        LineState::BlockComment(depth) => {
            let (end, open) = scan_block(&chars, 0, depth);
            out[..end].fill(Token::Comment);
            if open > 0 {
                return (out, LineState::BlockComment(open));
            }
            i = end;
        }
        LineState::Str => {
            let (end, closed) = scan_str(&chars, 0);
            out[..end].fill(Token::Str);
            if !closed {
                return (out, LineState::Str);
            }
            i = end;
        }
        LineState::RawStr(hashes) => {
            let (end, closed) = scan_raw(&chars, 0, hashes as usize);
            out[..end].fill(Token::Str);
            if !closed {
                return (out, LineState::RawStr(hashes));
            }
            i = end;
        }
    }

    while i < n {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '/' if next == Some('/') => {
                out[i..].fill(Token::Comment);
                i = n;
            }
            '/' if next == Some('*') => {
                let (end, open) = scan_block(&chars, i + 2, 1);
                out[i..end].fill(Token::Comment);
                if open > 0 {
                    return (out, LineState::BlockComment(open));
                }
                i = end;
            }
            '"' => {
                let (end, closed) = scan_str(&chars, i + 1);
                out[i..end].fill(Token::Str);
                if !closed {
                    return (out, LineState::Str);
                }
                i = end;
            }
            '\'' => {
                if let Some(end) = char_literal_end(&chars, i) {
                    out[i..end].fill(Token::Str);
                    i = end;
                } else if next.is_some_and(is_ident_start) {
                    let mut end = i + 1;
                    while end < n && is_ident_char(chars[end]) {
                        end += 1;
                    }
                    out[i..end].fill(Token::Meta);
                    i = end;
                } else {
                    i += 1;
                }
            }
            '#' if next == Some('[') || (next == Some('!') && chars.get(i + 2) == Some(&'[')) => {
                let mut j = if next == Some('[') { i + 1 } else { i + 2 };
                let mut depth = 0u32;
                let mut end = n;
                while j < n {
                    match chars[j] {
                        // Skip quoted strings so a `]` inside one does not end
                        // the attribute; an unterminated one runs to the end
                        // of the line, as does an unmatched `[`.
                        '"' => {
                            j = scan_str(&chars, j + 1).0;
                            continue;
                        }
                        '[' => depth += 1,
                        ']' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                end = j + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                out[i..end].fill(Token::Meta);
                i = end;
            }
            c if c.is_ascii_digit() => {
                i = scan_number(&chars, i, &mut out);
            }
            c if is_ident_start(c) => {
                // String-like prefixes only count at the start of a word.
                if c == 'b' && next == Some('"') {
                    let (end, closed) = scan_str(&chars, i + 2);
                    out[i..end].fill(Token::Str);
                    if !closed {
                        return (out, LineState::Str);
                    }
                    i = end;
                    continue;
                }
                if c == 'b'
                    && next == Some('\'')
                    && let Some(end) = char_literal_end(&chars, i + 1)
                {
                    out[i..end].fill(Token::Str);
                    i = end;
                    continue;
                }
                if c == 'r' || (c == 'b' && next == Some('r')) {
                    let mut j = i + if c == 'r' { 1 } else { 2 };
                    let hashes_start = j;
                    while j < n && chars[j] == '#' {
                        j += 1;
                    }
                    let hashes = j - hashes_start;
                    if j < n && chars[j] == '"' {
                        let (end, closed) = scan_raw(&chars, j + 1, hashes);
                        out[i..end].fill(Token::Str);
                        if !closed {
                            let h = u32::try_from(hashes).unwrap_or(u32::MAX);
                            return (out, LineState::RawStr(h));
                        }
                        i = end;
                        continue;
                    }
                    if c == 'r' && hashes == 1 && chars.get(j).copied().is_some_and(is_ident_start) {
                        // Raw identifier: r#type
                        while j < n && is_ident_char(chars[j]) {
                            j += 1;
                        }
                        i = j;
                        continue;
                    }
                }
                let mut end = i + 1;
                while end < n && is_ident_char(chars[end]) {
                    end += 1;
                }
                let token = classify_word(&chars[i..end]);
                if chars.get(end) == Some(&'!') && chars.get(end + 1) != Some(&'=') {
                    end += 1;
                    out[i..end].fill(Token::Meta);
                } else {
                    out[i..end].fill(token);
                }
                i = end;
            }
            _ => i += 1,
        }
    }
    (out, LineState::Normal)
}

/// Mark the number starting at `start` and return the index after it.
fn scan_number(chars: &[char], start: usize, out: &mut [Token]) -> usize {
    let n = chars.len();
    let mut i = start;
    let radix_prefix = chars[i] == '0' && matches!(chars.get(i + 1), Some('x' | 'b' | 'o'));
    if radix_prefix {
        i += 2;
    } else {
        while i < n && (chars[i].is_ascii_digit() || chars[i] == '_') {
            i += 1;
        }
        // A dot belongs to the number only when a digit follows.
        if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
            while i < n && (chars[i].is_ascii_digit() || chars[i] == '_') {
                i += 1;
            }
        }
        // Exponent: e10, E+3, e-3.
        if matches!(chars.get(i), Some('e' | 'E')) {
            let mut j = i + 1;
            if matches!(chars.get(j), Some('+' | '-')) {
                j += 1;
            }
            if chars.get(j).is_some_and(|c| c.is_ascii_digit()) {
                i = j;
                while i < n && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    i += 1;
                }
            }
        }
    }
    // Hex digits, `_` and a type suffix (u8, f32, usize).
    while i < n && is_ident_char(chars[i]) {
        i += 1;
    }
    out[start..i].fill(Token::Number);
    i
}

#[cfg(test)]
mod tests;
