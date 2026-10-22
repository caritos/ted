# Rust Syntax Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Files opened with a `.rs` extension are drawn with syntax colors in the editor (keywords, types, strings, comments, numbers, macros/attributes/lifetimes). Everything else renders exactly as today.

**Architecture:** A pure, hand-written tokenizer in a new library module (`highlight.rs`) classifies every character of a line and carries a small state across lines (block comments, multi-line strings, raw strings). `Editor::view` asks it for the visible rows and returns a per-character style grid next to the existing text lines; `App` groups runs of equal style into `Span`s; `main.rs` maps the new `Style` variants to terminal colors.

**Tech Stack:** Rust 2024, crossterm 0.29, std only. No new dependencies (no syntect, no tree-sitter).

**Spec:** none written; the user asked "this is a rust file, can we have syntax highlighting" (with a screenshot of `src/main.rs` open in `ted` with no colors). Design decisions are the controller's calls and are listed below.

## Decisions (controller's calls)
- Hand-written tokenizer, no dependency. A `Language` enum (`Rust` only for now) chosen by file extension via `Language::from_path`; adding a language later means adding a variant and a tokenizer.
- Always on for known extensions; no configuration, no toggle. Unnamed buffers and unknown extensions get no highlighting (all `Style::Normal`).
- Colors use the terminal's ANSI palette so they follow the user's theme: keyword magenta, type yellow, string green, comment dark grey, number cyan, meta (macros, attributes, lifetimes) blue.
- Highlight state is recomputed by scanning from line 0 to the first visible row on every frame (no cache yet). Redraws only happen when the screen changes, so this is acceptable for normal files; a cache is deferred.
- The git view's diff and the file tree are not highlighted (deferred).

## Global Constraints
- TDD: failing test first; logic in the library crate, `main.rs` thin. `cargo test` and `cargo clippy --all-targets` clean; no new dependencies. If `cargo` is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Real-terminal check: `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted` must pass.
- Existing behavior is unchanged: editing, scrolling, cursor, status bar, quit rules, git view, file tree. Text content and cursor positions must be byte-for-byte what they are today; highlighting only adds styles.
- The tokenizer works on `char`s, never on byte indices; it never panics on any input (unterminated strings/comments, empty lines, emoji, very long lines, invalid sequences).
- Style grids are exactly as wide as the clipped text of each row (one entry per displayed char), including after horizontal scrolling.
- Never call the real clock; no file system access in the tokenizer.
- Update `README.md` and the dated log and layout in `CLAUDE.md`; claim nothing as hand-verified (colors on a real terminal are NOT yet verified by hand).

## Rust token rules (the contract for Task 1)
Every character of a line gets exactly one `Token`: `Plain`, `Keyword`, `Type`, `Str`, `Comment`, `Number`, `Meta`.
- Line comments: `//` to end of line (includes `///`, `//!`) are `Comment`.
- Block comments: `/* ... */`, nested, may span lines (carry depth in the state), are `Comment`. An unterminated one stays a comment to the end of the file.
- Strings: `"..."` with `\` escapes (an escaped quote does not end it) may span lines; `b"..."` too. The whole literal including quotes and the `b` prefix is `Str`. Unterminated: `Str` to the end of the file.
- Raw strings: `r"..."`, `r#"..."#`, `r##"..."##`, and `br...` forms; no escapes; may span lines; ends at a quote followed by the same number of `#`. Whole literal `Str`.
- Char literals vs lifetimes: after `'`, if the next char is `\` it is a char literal up to the closing `'` (handles `'\''`, `'\n'`, `'\u{1F600}'`); else if the char after next is `'` it is a char literal (`'a'`, `'é'`); else it is a lifetime/label (`'a`, `'static`, `'_`) made of `'` plus the identifier chars and is `Meta`. Byte chars `b'x'` are `Str`. Char literals are `Str`.
- Numbers: decimal, `0x`/`0b`/`0o` with hex digits, `_` separators, floats (`1.5`, `1e10`, `1.5e-3`), and type suffixes (`u8`, `f32`, `usize`); all `Number`. A `.` is part of a number only when followed by a digit; so `1..2` is `1`, `..`, `2`, and `1.max(2)` is `1`, `.`, `max`.
- Identifiers (start with a letter or `_`, continue with letters/digits/`_`; unicode letters allowed): a keyword from this list is `Keyword`: `as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self static struct super trait true type unsafe use where while`. `Self` is `Type`. An identifier starting with an uppercase letter and containing at least one lowercase letter (`Vec`, `PathBuf`, `Style`) is `Type`. ALL_CAPS constants and all other identifiers are `Plain`. An identifier immediately followed by `!` (not `!=`) is `Meta` including the `!` (`eprintln!`, `vec!`). Raw identifiers `r#type` are `Plain`.
- Attributes: `#[` or `#![` starts `Meta` that runs through the matching `]` (tracking nested `[`/`]` on that line) or to the end of the line if unmatched.
- Everything else (operators, punctuation, whitespace) is `Plain`.
- State carried across lines (`LineState`): `Normal`, `BlockComment(depth)`, `Str`, `RawStr(hashes)`. `highlight_line(language, state, line) -> (Vec<Token>, LineState)` returns one `Token` per `char` of `line` and the state at the start of the next line.

## Review Focus
1. Unterminated strings/block comments at end of file, and a `"` inside a comment or a `//` inside a string.
2. `'` lifetimes versus char literals: `'a'`, `'\''`, `<'a>`, `&'static str`, `b'x'`, `'é'`.
3. Raw strings with hashes spanning several lines; nested block comments.
4. Numbers versus ranges and method calls: `1..2`, `1.max(2)`, `0xFFu8`, `1e-3`, `1_000`.
5. Style/char alignment under horizontal scrolling, emoji and CJK in strings, and very long lines; non-`.rs` files unchanged; performance of the per-frame scan on a large file.

## File Structure
- `src/highlight.rs` (create) + `src/highlight/tests.rs`: `Language`, `Token`, `LineState`, `highlight_line`.
- `src/screen.rs` (modify): add `Style::Keyword`, `Style::TypeName`, `Style::Str`, `Style::Comment`, `Style::Number`, `Style::Meta`.
- `src/editor.rs` (modify) + `src/editor/tests.rs`: `View` gains `styles`.
- `src/app.rs` (modify) + `src/app/tests.rs`: group per-char styles into spans.
- `src/main.rs` (modify): colors for the new styles.
- `src/lib.rs` (modify): `pub mod highlight;`.
- `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (modify).

---

### Task 1: The Rust tokenizer

**Files:**
- Create: `src/highlight.rs`, `src/highlight/tests.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces in `ted::highlight`:
  - `enum Language { Rust }` (`Debug, Clone, Copy, PartialEq, Eq`) with `fn from_path(path: &Path) -> Option<Language>` (extension `rs`, case-sensitive; no extension or anything else gives `None`).
  - `enum Token { Plain, Keyword, Type, Str, Comment, Number, Meta }` (`Debug, Clone, Copy, PartialEq, Eq`).
  - `enum LineState { Normal, BlockComment(u32), Str, RawStr(u32) }` (`Debug, Clone, Copy, PartialEq, Eq`) with `impl Default` giving `Normal`.
  - `fn highlight_line(language: Language, state: LineState, line: &str) -> (Vec<Token>, LineState)` following the "Rust token rules" section exactly.

- [ ] **Step 1: Write failing tests** in `src/highlight/tests.rs` using a helper that renders a line as a compact string, one letter per char (`.` Plain, `k` Keyword, `t` Type, `s` Str, `c` Comment, `n` Number, `m` Meta) so assertions are readable, e.g. `assert_eq!(render("let x = 5;"), "kkk.......n.")`. Cover at least:
  - `from_path_recognizes_rs_only` (`main.rs`, `/a/b/lib.rs` yes; `README.md`, `Makefile`, `x.RS`, `rs` no)
  - keywords and `Self`, `self` (`fn main() {}`), uppercase types (`Vec<PathBuf>`), ALL_CAPS constants and lowercase identifiers stay Plain, identifiers containing keywords (`format`, `letter`, `mod_name`) stay Plain
  - macros: `eprintln!("x")` (name and `!` Meta, the string Str), `a != b` is not a macro
  - line comments including `///` and `//!`; `//` inside a string stays Str; `"` inside a comment stays Comment
  - nested block comments across lines with the carried depth (`/* a /* b */ still */ x` and a three-line version checking the returned state each line); unterminated block comment keeps `BlockComment` state at the end
  - strings with escapes (`"a\"b"`), multi-line strings carrying `Str` state, unterminated string, byte strings
  - raw strings: `r"x"`, `r#"a"b"#`, `r##"a"#b"##`, multi-line raw strings carrying `RawStr(n)`, `br"..."`, and `r#type` as a raw identifier (Plain)
  - char literals and lifetimes: `'a'`, `'\''`, `'\n'`, `'\u{1F600}'`, `'é'`, `b'x'`, `<'a>`, `&'static str`, `'_`, a loop label `'outer: loop`
  - numbers: `42`, `1_000`, `0xFFu8`, `0b1010`, `0o777`, `1.5`, `1e10`, `1.5e-3f32`, `1..2` (two numbers around Plain `..`), `1.max(2)`
  - attributes: `#[derive(Debug, Clone)]`, `#![allow(dead_code)]`, nested brackets `#[cfg(any(a, b))]`, unmatched `#[` runs to end of line
  - unicode: emoji and CJK in strings and comments give one Token per `char` (length of the result equals `line.chars().count()`); an empty line gives an empty vec and unchanged state
  - `never_panics_on_odd_input`: a loop over a list of tricky fragments (`'`, `"`, `r#`, `r#"`, `/*`, `*/`, `\`, `0x`, `1e`, `#[`, `b'`, a lone combining char, a 100k-char line) run through `highlight_line` from every `LineState`, asserting the result length equals the char count.
  - `state_carries_over_many_lines`: highlight a small multi-line program line by line and assert the full rendering.
- [ ] **Step 2: Run `cargo test highlight::` and confirm FAIL.**
- [ ] **Step 3: Implement `src/highlight.rs`** (with `#[cfg(test)] mod tests;` at the bottom, as in `editor.rs`). Work on a `Vec<char>` of the line with an index cursor; no regexes, no dependencies.
- [ ] **Step 4: Run `cargo test` and `cargo clippy --all-targets`; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Add Rust syntax tokenizer"`.

---

### Task 2: Draw highlighted rows

**Files:**
- Modify: `src/screen.rs`, `src/editor.rs`, `src/editor/tests.rs`, `src/app.rs`, `src/app/tests.rs`, `src/main.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `Style` gains `Keyword`, `TypeName`, `Str`, `Comment`, `Number`, `Meta` (update every exhaustive `match` on `Style`).
  - `View` (in `src/editor.rs`) gains `pub styles: Vec<Vec<Style>>`: one inner vec per entry of `lines`, with exactly one `Style` per char of that (already clipped) line string. Rows for non-Rust buffers, empty buffer rows and the `~` filler rows are all `Style::Normal`.
  - `Editor::view` obtains the language from `Language::from_path(self.buffer.path()?)`; for a highlighted language it walks the lines from line 0 carrying `LineState` up to the first visible row, then tokenizes each visible line and clips the token vector with the same `scroll_col` and `width` as the text so styles stay aligned with the displayed chars. Token to Style: Plain to Normal, Keyword to Keyword, Type to TypeName, Str to Str, Comment to Comment, Number to Number, Meta to Meta.
  - `App`'s conversion of `Editor::view` to a `Screen` groups consecutive chars of equal style in a row into one `Span` (a row of all-Normal chars is still exactly one `Normal` span, as today).
  - `main.rs` draws the new styles with the terminal's ANSI colors: Keyword magenta, TypeName yellow, Str green, Comment dark grey, Number cyan, Meta blue; attributes and colors are reset after each span like the existing styles.

- [ ] **Step 1: Write failing tests:**
  - `editor_view_styles_have_one_entry_per_displayed_char` for a Rust buffer and a non-Rust buffer, over several widths and after horizontal scrolling
  - `rust_buffer_styles_mark_keywords_strings_and_comments` (a named `.rs` buffer, check specific columns)
  - `non_rust_and_unnamed_buffers_are_all_normal`
  - `multiline_block_comment_state_carries_into_visible_rows_when_scrolled_down` (open a buffer whose block comment starts above the first visible row)
  - `app_groups_equal_styles_into_spans_and_keeps_text_identical` (the concatenated span text equals the plain line text; all-normal rows are a single span)
  - `existing editor and app tests keep passing unchanged`
- [ ] **Step 2: Run `cargo test` for the new tests and confirm FAIL.**
- [ ] **Step 3: Implement the `Style` variants, `View.styles`, the span grouping and the `main.rs` color mapping.**
- [ ] **Step 4: Run `cargo test`, `cargo clippy --all-targets`, `cargo build --release`, and the pty smoke test; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Draw Rust files with syntax colors"`.

---

### Task 3: pty smoke test and docs

**Files:**
- Modify: `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md`

- [ ] **Step 1: Extend `scripts/pty_smoke.py`** (same helpers, hermetic temp dirs, cleanup in `finally`): write a small `.rs` file to a temp dir (for example `fn main() { let s = "hi"; } // note`), open it with `ted <file>` and assert the raw pty output contains the expected SGR color sequences adjacent to the right text (magenta before `fn`, green before the string, dark grey before the comment; match on the escape sequences crossterm emits, check the real bytes first), and that a `.txt` file with the same text contains none of those color sequences. Confirm the new checks FAIL with the highlighting stubbed out (for example `Language::from_path` returning `None`), then restore exactly; never commit a stub.
- [ ] **Step 2: Docs.** README: syntax highlighting for `.rs` files, the colors, always on, no config, other languages plain for now. `CLAUDE.md`: Layout line for `highlight.rs`, update the `editor.rs`/`screen.rs` lines if stale, and a dated 2026-10-22 log entry (Done unit-tested / Done pty smoke listed separately; Not yet verified by hand: the colors on a real terminal and theme readability (especially dark grey comments), a very large Rust file for scroll speed; Deferred: other languages, highlight state cache, highlighting in the git diff and the file tree, a theme or toggle).
- [ ] **Step 3: Full clean verification:** `cargo clean && cargo test && cargo clippy --all-targets && cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`.
- [ ] **Step 4: Commit** `git commit -m "Cover syntax highlighting in pty smoke test; update docs"`.
