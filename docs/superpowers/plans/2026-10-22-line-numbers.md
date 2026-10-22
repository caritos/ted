# Line Numbers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox syntax.

**Goal:** Show line numbers in the editor's left gutter; F2 shows/hides them.

**Architecture:** `Editor::view` prefixes each text row with a gutter (right-aligned 1-based number + one space) and the matching `Style::LineNo` styles; the cursor column and horizontal scroll account for the gutter; the highlight styles still map only to the text chars. `App` owns the toggle state (shared by all buffers, not persisted) and passes it to each editor before drawing (same pattern as `set_tag`). `main.rs` maps the new style (dim).

**Tech Stack:** Rust, crossterm (existing). No new dependencies.

**Spec:** none; design calls below (user: "how do i show/hide line numbers").

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` only gets the style mapping.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- `View.lines` stays "exactly the visible text rows, clipped to the width, sanitized"; `View.styles` stays one style per char of each line (gutter chars included); `View.cursor` stays (row, col) within `lines` (gutter included).
- Tiny sizes (0..=6 x 0..=6) must not panic; the gutter must never push the cursor outside `width`.
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls (user may veto)

- Default on. F2 toggles in the editor; F2 is ignored in the git view, tree, buffer list and help, and while a Find/Go-to prompt is open (the prompt swallows it). One setting for all buffers, not persisted.
- Gutter = number right-aligned in `max(3, digits(line_count))` columns, then one space. Example: `  1 text`, ` 42 text`, `1000 text` (4 digits + space once the buffer has 1000+ lines). Rows past the end of the buffer show `~` as today with a blank gutter. Number width never shrinks while scrolling (it depends on `line_count`, not the visible rows).
- The gutter is hidden (as if off) when `width < gutter_width + 10`, so a narrow terminal keeps its text.
- Gutter style: new `Style::LineNo`, drawn with the Dim attribute (like comments). The current line's number is not highlighted (deferred).
- Horizontal scroll and the width used for clipping/scroll apply to the text area (`width - gutter`), so long lines scroll as before. Prompt/messages/status are not offset.
- Search/go-to-line/cursor logic is unchanged (columns stay text columns).

## Review Focus

- Cursor lands on the right char with the gutter on, off, after toggling mid-session, after horizontal scroll, at 1/9/10/99/100/999/1000 lines (gutter width changes), with CRLF/tab/hostile text (sanitized one-for-one).
- Highlight styles stay aligned with text (syntax colors on a `.rs` file with the gutter on).
- Toggle with a pending warning/prompt; toggle in other modes ignored.
- Tiny terminals and widths around the hide threshold (gutter_width+9 / +10).

---

### Task 1: Gutter in `Editor::view`, `Style::LineNo`, F2 state

**Files:** Modify `src/editor.rs`, `src/screen.rs`, `src/main.rs`, `src/editor/tests.rs` (or a new `src/editor/gutter_tests.rs` declared in `editor.rs` like `prompt_tests.rs`).

**Interfaces — Produces:** `Style::LineNo`; `Editor::set_line_numbers(&mut self, on: bool)`; (App will call it each frame) `View` shape unchanged. `main.rs` draws `LineNo` with the Dim attribute exactly like `Style::Comment` is drawn.

- [ ] **Step 1: Failing tests:** gutter text/width at 5, 9, 10, 999, 1000 lines; `~` rows have a blank gutter; styles for gutter chars are `LineNo` and text styles are unchanged and aligned (a `.rs` buffer with a keyword); cursor column includes the gutter and tracks horizontal scroll (a long line scrolled right: scroll applies to the text area); off → output identical to the pre-change layout (assert against the existing expectations); hide threshold at `width = gutter + 9` vs `+ 10`; 0..=6 x 0..=6 sweep with the gutter on and off; hostile text still sanitized one-for-one.
- [ ] **Step 2:** FAIL. **Step 3:** implement. **Step 4:** `cargo test` + clippy. **Step 5:** commit `Editor: line number gutter`.

### Task 2: App toggle (F2), help, docs, pty smoke

**Files:** Modify `src/app.rs`, `src/app/tests/` (follow the existing per-topic test modules), `src/help.rs`, `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md`.

- [ ] **Step 1: Failing tests:** F2 toggles in the editor and the next `screen()` has/has no gutter in every buffer; F2 ignored in git view/tree/buffer list/help and with a prompt open; the setting survives switching buffers; help lists F2 (Ctrl-S/Ctrl-Q still on the first 80x24 screen).
- [ ] **Step 2:** FAIL. **Step 3:** implement (App holds `line_numbers: bool = true`, sets it on the current editor before each draw). **Step 4:** tests + clippy.
- [ ] **Step 5:** pty smoke: a 12-line file shows `  1 ` and `  12 `-style gutter text on start (assert on the exact rendered text `  1 line 0` for a file whose lines are `line N`), F2 removes it (the raw `  1 line` no longer appears after redraw, `line 0` still does), F2 again restores it. Show it FAILING against the installed `~/.cargo/bin/ted`.
- [ ] **Step 6:** README (key table + a "Line numbers" note) and CLAUDE.md (layout line, dated log: done/unit-tested, pty, not hand-verified (dim gutter look), deferred: current-line highlight, relative numbers, per-file setting, persisted setting; real test count). Commit `Line numbers: F2 toggle, help, docs`.
