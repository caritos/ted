# Search and Go-to-Line Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox syntax.

**Goal:** Find text in the current buffer (Ctrl-F, incremental, smart case, wrapping, next/previous) and jump to a line number (Ctrl-L).

**Architecture:** `Buffer` gets pure search/jump primitives. `Editor` owns an optional one-line prompt (`Prompt { kind: Find|Goto, input, origin cursor }`) drawn in the message row; while it is open every key goes to it. `App` routes keys to the editor's prompt first and keeps Ctrl-Q. No new screen mode, no change to `View`/`main.rs`.

**Tech Stack:** Rust, crossterm, ropey (existing). No new dependencies.

**Spec:** none; design calls below (user: "let's work on search and go to line").

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` untouched.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- All drawn text (prompt included) goes through `editor.rs`'s `clip`; tiny sizes (0..=6 x 0..=6) must not panic. Searching never mutates the buffer or marks it modified.
- A buffer's cursor must always be a valid (row, col) afterwards (see `Buffer::place_cursor`; lone `\r`, CRLF and other ropey line breaks exist).
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls (user may veto)

- Ctrl-F opens `Find: ` in the message row (`Find: foo_` with a trailing underscore). Typing searches incrementally from the cursor position where the prompt opened (the match start becomes the cursor; the view scrolls to it). Backspace edits the query and re-searches from the origin. Enter accepts (prompt closes, cursor stays on the match, the term is remembered). Esc cancels (cursor and scroll back to where they were). Down or Ctrl-F inside the prompt: next match after the current one; Up: previous match; both wrap, showing `Wrapped` in the message after the prompt closes is not required, but the prompt line shows `Find: foo (wrapped)` while it applies. No match: the cursor stays and the prompt shows `Find: foo (not found)`.
- Smart case: the search is case-insensitive unless the query contains an uppercase char. Per-char comparison with `to_lowercase().next()` is enough; do not change string lengths. Empty query: no search. Queries cannot contain newlines (Enter ends the prompt).
- Pressing Ctrl-F with an empty prompt after a previous search prefills the remembered term (as typed text, searchable immediately).
- F3 outside the prompt: next match for the remembered term (message `Not found: foo` if none, `No previous search` if none remembered).
- Ctrl-L opens `Go to line: ` accepting only ASCII digits (other chars ignored). Enter jumps to that 1-based line, column 0, clamped to the last line (0 or empty input: Esc-like no-op close). Esc cancels.
- While a prompt is open, `App` sends it every key, except Ctrl-Q, which cancels the prompt and then follows the normal quit rules. Ctrl-F/Ctrl-L are ignored outside the editor mode (git view, tree, buffer list, help).
- The status bar and cursor position are unchanged; the cursor is the match start. A separate match highlight is deferred.

## Review Focus

- Matches on the cursor's own line before/after the cursor, at line ends, on the last line, in files without a trailing newline, with CRLF and lone-CR breaks, and non-ASCII text (column = char index).
- Wrap-around in both directions and a single-match buffer (next lands on the same match).
- Esc really restores cursor and scroll; Enter keeps them; backspacing the whole query returns to the origin.
- A prompt open while the buffer is modified never changes `is_modified`; the quit warning and prompt interplay (Ctrl-Q cancels the prompt first).
- Hostile query text (escape sequences typed or pasted) is sanitized in the message row.

---

### Task 1: Buffer search and jump primitives

**Files:** Modify `src/buffer.rs`, `src/buffer/tests.rs`.

**Interfaces — Produces:**
- `Buffer::find(&self, query: &str, from: (usize, usize), forward: bool, include_from: bool) -> Option<Found>` where `Found { row: usize, col: usize, wrapped: bool }` (pub struct, `Debug, PartialEq, Eq, Clone, Copy`). Forward: first match whose start is at or after `from` (strictly after when `include_from` is false), wrapping to the top. Backward: last match whose start is strictly before `from` (at-or-before is not offered), wrapping to the bottom. Smart case as in Design calls. `col` is a char index within the line text (without its break).
- `Buffer::set_cursor(&mut self, row: usize, col: usize)`: clamps to a valid position, resets the goal column, no dirty/modified change.
- `Buffer::go_to_line(&mut self, line_1_based: usize)`: clamps; column 0.

- [ ] **Step 1: Failing tests:** forward/backward/wrap/include_from; smart case both ways; non-ASCII column; CRLF/lone-CR/no trailing newline; query longer than any line; empty query returns None; `set_cursor` clamping (row past end, col past line end) keeps the cursor valid and `is_modified` false; `go_to_line(0)`, `go_to_line(2)`, `go_to_line(999)`.
- [ ] **Step 2:** run, see FAIL. **Step 3:** implement (per-line `line(r)` chars; single-line matching only). **Step 4:** tests + clippy pass. **Step 5:** commit `Buffer: find and go-to-line primitives`.

### Task 2: Editor prompt (Find and Go to line)

**Files:** Modify `src/editor.rs`, `src/editor/tests.rs`.

**Interfaces — Consumes:** Task 1. **Produces:** `Editor::in_prompt(&self) -> bool`, `Editor::cancel_prompt(&mut self)`; `Editor::handle_key` routes every key to the prompt while it is open; Ctrl-F, Ctrl-L, F3 handled in the editor when no prompt is open. Prompt text is built in `view()` into the message row (sanitized by `clip`).

- [ ] **Step 1: Failing tests** for every Design call: incremental typing moves the cursor and scrolls (check `view()` shows the match line); Backspace to empty restores origin; Esc restores cursor and scroll; Enter keeps; Down/Ctrl-F next, Up previous, wrapping text, not-found text; remembered term prefill and F3 (and both F3 messages); Ctrl-L digits-only, Enter jump, clamp, Esc; Ctrl-S/other Ctrl keys are swallowed inside the prompt (buffer unchanged, not modified); typed text never edits the buffer; hostile query is sanitized in `View.message`; 0..=6 x 0..=6 sweep with the prompt open.
- [ ] **Step 2:** FAIL. **Step 3:** implement. **Step 4:** pass + clippy. **Step 5:** commit `Editor: Ctrl-F find and Ctrl-L go-to-line prompts`.

### Task 3: App routing, help, docs, pty smoke

**Files:** Modify `src/app.rs`, `src/app/tests.rs` (or `src/app/tests/` submodule file as the repo does), `src/help.rs`, `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md`.

**Interfaces — Consumes:** `Editor::in_prompt`, `Editor::cancel_prompt`.

- [ ] **Step 1: Failing tests:** while a prompt is open Ctrl-G/T/O/N/P/W/F1/`?` do nothing but go to the prompt (typed `?` lands in the query; Ctrl-G does not open the git view); Ctrl-Q cancels the prompt then warns/quits per the app rules; Ctrl-F/Ctrl-L in git view/tree/list/help do not open a prompt; F3 works from the editor only; the help table lists Ctrl-F, Ctrl-L, F3 and the prompt keys (and the first-screen test still shows Ctrl-S/Ctrl-Q).
- [ ] **Step 2:** FAIL. **Step 3:** implement routing before the global-key handling in `App::handle_key`. **Step 4:** `cargo test` + clippy.
- [ ] **Step 5:** pty smoke scenario (200-line file `line N`): Ctrl-F, type `line 150`, Enter → screen shows `line 150`; Ctrl-L `10` Enter → shows `line 9`; Ctrl-F `zzz` → `not found`; Esc → back. Show it FAILING against the installed `~/.cargo/bin/ted` and passing on the release build.
- [ ] **Step 6:** README (key table rows, a Search section) and CLAUDE.md (layout/log with honest done/unit-tested/pty/not-hand-verified/deferred: match highlight, replace, regex, multi-line queries, search in other views; real test count); commit `Search and go-to-line: routing, help, docs`.
