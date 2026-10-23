# Replace and Match Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ctrl-R replaces every occurrence of a term (in the selection if there is one, else the whole buffer) as one undo step, and while the Find prompt is open every visible match is highlighted.

**Architecture:** `Buffer` gets `matches_in_line` (the same single-line, smart-case matching `find` uses, exposed per row) and `replace_all` (one undo `Edit` covering the span from the first to the last replaced match, through the existing range-replace used by `insert_str`). `Editor` draws `Style::Match` on visible matches while a Find prompt is open and runs a two-step `Replace` / `With` prompt on the existing prompt machinery.

**Tech Stack:** Rust, existing crates only.

**Spec:** none; design calls below (user may veto).

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` only maps the new `Style::Match` to an attribute.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted` (can take several minutes: generous timeout). If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Cursor columns char-based, every re-placement through `place_cursor`; every changed row `mark_dirty`ed (highlight cache). All drawn text sanitized one-for-one. Replacement text is stored raw in the buffer (it is typed into a prompt, so it is single-line).
- Undo: the whole replace is exactly one undo step; `is_modified` follows the existing history rules; never replace into a buffer with zero matches (no history entry, not modified).
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls

- Matching is exactly `find`'s: single-line, smart case (case-insensitive unless the term has an uppercase char), non-overlapping left to right per line. The replacement text is inserted as typed (no case adaptation).
- Highlight: only while a Find prompt is open (typing, stepping with Down/Up/Ctrl-F); all matches in the visible rows get `Style::Match` (underline in `main.rs`); the current match is the cursor position, and a selection/`Selected` style wins over `Match`; syntax colours are replaced on matched cells only by the underline attribute (Match must combine with colours, or if the style model cannot combine, Match wins on those cells). Cleared when the prompt closes. Cost: only visible rows are scanned per frame.
- Replace: Ctrl-R (editor mode, not while a prompt is open) opens `Replace: ` prefilled with the shared search term (same as Ctrl-F prefill; typed text is appended; Backspace edits). Enter with a non-empty term moves to `With: ` (empty allowed = delete the matches). Enter performs the replacement. Esc at either step cancels, restoring cursor, scroll and selection. An empty term + Enter does nothing (back to normal). The `replace` command opens the same prompt.
- Scope: inside the selection (matches must lie fully inside it) when one exists at the time Ctrl-R is pressed, else the whole buffer. After replacing, the selection is cleared, the cursor goes to the start of the first replacement, and the replace term becomes the shared search term (`App` already pulls it via `take_search`).
- Messages: `Replaced N occurrence(s)` (`1 occurrence`), `Not found: <term>`; replacing identical text (`a` with `a`) still counts and still makes one undo step.
- Out of scope: per-match confirm (y/n), regex, case-preserving replace, multi-line terms, replace in the views, highlight after the prompt closes.

## Review Focus

- Matches adjacent to each other (`aa` in `aaaa`), matches at line start/end, replacement containing the term (`a` -> `aa`, no re-scan loops), empty replacement, term that spans a tab or multibyte chars, lone `\r`/`\r\n` text; huge buffers (100k lines, many matches: one pass, no quadratic edits, one undo step, undo restores exactly, modified flag).
- Selection scope: partial matches at the selection edges are not replaced; a selection that ends mid-match; undo restores text, cursor and the buffer is unmodified again if it was clean.
- Prompt flow: Esc at either step restores everything; Ctrl-Q cancels like other prompts; keys in views/help do nothing new; tiny sizes 0..=16 x 0..=6 with highlights and both prompt steps open: no panic.
- Highlight alignment with tabs, horizontal scroll and the gutter; selection over a match; a long line with thousands of matches stays fast per frame.

---

### Task 1: `Buffer::matches_in_line` and `Buffer::replace_all`

**Files:** Modify `src/buffer.rs`; tests in a new `src/buffer/replace_tests.rs` (wire like `selection_tests.rs`).

**Interfaces:**
- Produces `Buffer::matches_in_line(&self, row: usize, query: &str) -> Vec<(usize, usize)>` — (col, len in chars) of every non-overlapping smart-case match in that row's text, in order; empty for an empty query or out-of-range row; must reuse `find`'s matching code (no second implementation of smart case).
- Produces `Buffer::replace_all(&mut self, query: &str, with: &str, within: Option<((usize, usize), (usize, usize))>) -> usize` — replaces every non-overlapping match (inside `within` if given: a match must lie fully inside; positions are (row, col) in document order); returns the count; zero matches changes nothing (no history entry, not modified, selection untouched); otherwise one undo `Edit` over the span first-match-start..last-match-end, cursor ends at the start of the first replacement, selection cleared, dirty rows marked from the first replaced row (one above if the text involved line breaks), `merge_ok` off.

- [ ] **Step 1: Failing tests:** matches_in_line: smart case both ways, adjacent non-overlapping (`aa` in `aaaa` = cols 0, 2), multibyte and tab, out-of-range row; replace_all: count and text for simple, adjacent, replacement containing the term (no loops), empty replacement, term at line start/end, several lines, `\r\n` text, within-selection (partial edge matches not replaced), zero matches leaves history/modified untouched; undo restores text/cursor/modified in one step, redo reapplies; a randomized test against a naive reference (random texts/terms/selections; text equals the reference, undo restores, count equals the number of reference matches); performance guard: 100k lines with many matches replaced in well under a second in a debug test run, and the history holds one step.
- [ ] **Step 2:** `cargo test` — confirm failures are for the right reason.
- [ ] **Step 3:** Implement (one pass over the rope lines to collect match ranges, build the replaced span as one `String`, apply through the existing range-replace used by `insert_str`).
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** Commit `Buffer: match positions per line and replace all`.

### Task 2: Highlight, Replace prompt, command, help, docs, pty

**Files:** Modify `src/editor.rs` (Style assignment in view for visible rows while a Find prompt is open; `PromptKind::Replace` with two steps; Ctrl-R), `src/screen.rs` (`Style::Match`), `src/main.rs` (underline for `Style::Match`, thin), `src/command.rs` (`replace` command; check prefix ambiguity: `re` becomes ambiguous with `redo`/`replace`: update the existing tests honestly and keep `redo` reachable by exact name and by `red`), `src/app.rs` (run the command by calling a public `Editor::open_replace`), `src/help.rs` (rows <= 80 columns; Ctrl-S/Ctrl-Q stay on the first 80x24 screen; pinned key-list test), `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (layout, dated log entry: done / not hand-verified / deferred), tests in `src/editor/replace_tests.rs`, `src/app/tests/`.

**Interfaces:** Consumes Task 1. Produces `Style::Match`, `Editor::open_replace(&mut self)`, Ctrl-R handling, prompt lines `Replace: {term}_` and `With: {text}_` (term shown in the second step as `With ({term}): {text}_` if it fits, else just `With: {text}_`).

- [ ] **Step 1: Failing tests:** view shows `Style::Match` on exactly the visible matches while the Find prompt is open (several rows, adjacent matches, tabs, horizontal scroll, gutter untouched), none after Enter/Esc, none with an empty term, `Selected` wins over `Match`; Ctrl-R flow: prefill from the shared term, typing, Enter -> `With:` step, Enter replaces and shows `Replaced N occurrences`, one Ctrl-Z restores, shared term updated (App level); Esc at either step restores cursor, scroll and selection; empty term Enter does nothing; scope = selection when one exists; `Not found: <term>` shows and nothing changes; the `replace` command and `re`/`red` prefix behaviour; Ctrl-R ignored in views/help and inside prompts; sweep 0..=16 x 0..=6 with highlights and both prompt steps open: no panic.
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** pty smoke (hermetic temp dir, 12-line file with several `foo` lines): Ctrl-F `foo` draws underlined spans (`\x1b[4m`) before Enter and none after; Ctrl-R, `foo`, Enter, `bar`, Enter shows `Replaced N occurrences`, the text now has `bar` and no `foo`, `[+]` in the status; Ctrl-Z restores `foo` lines; Esc during `With:` changes nothing; exit 0 via Ctrl-Q (second press if needed). Show the new checks failing against the previously installed `~/.cargo/bin/ted`, then passing on the release build.
- [ ] **Step 6:** Docs; commit `Replace (Ctrl-R) and match highlighting while searching`.
