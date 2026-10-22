# Help Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan. Steps use checkbox syntax.

**Goal:** A way to discover the commands from inside `ted`: F1 opens a scrollable help screen listing every key.

**Architecture:** New `src/help.rs` (pure state + a static table of sections/keys + render to `Screen`), a `Mode::Help` in `src/app.rs`. Modeled on `src/bufferlist.rs`/`src/filetree.rs`.

**Tech Stack:** Rust, crossterm (existing). No new dependencies.

**Spec:** none; design calls below (user asked: "i need a way to get help.. figure out the commands").

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` stays thin (no change expected).
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- All drawn text goes through `text::clip`/`pad`/`sanitize`; tiny sizes (0..=6 x 0..=6) must not panic.
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls (user may veto)

- F1 opens help from the editor, git view, file tree and buffer list (Ctrl-H is Backspace in a terminal, so not used). `?` also opens it from the three views (they have no text input).
- In help: Up/Down scroll a line, PageUp/PageDown a page, Home/End top/bottom; `q`, Esc or F1 return to the mode it was opened from (the editor if that is gone). Ctrl-Q still follows the app quit rules (warning shown in the help message row); Ctrl-N/P/O/G/T leave help and do what they do elsewhere; Ctrl-W ignored.
- The help text is a single `const` table in `help.rs`: sections Editor, Files and buffers, Views (git view, file tree, buffer list keys), Quit. It must list every key the README key tables list (Ctrl-S/Q/G/T/O/N/P/W, arrows, Home/End, PageUp/PageDown, Tab, Backspace/Delete, Enter, F1, plus the per-view keys: git view `r`, `b`, Enter, q/Esc; tree Right/Left/Enter; buffer list Enter).
- A one-time hint: the editor's initial message row shows `F1: help` until the first key press (use the existing message mechanism; any key clears it as messages do today).
- Status row of help: ` Help  (q to close)`; no cursor.

## Review Focus

- Help opened from each of the 4 places returns to the same place; opening while a quit/close warning is armed cancels it like any other key.
- Scroll never goes past the table; tiny terminals and long lines (clipped, not wrapped) do not panic.
- Every key in the README key tables appears in the help table (test it against a hard-coded list of key names in the test, not by parsing README).
- The `F1: help` hint does not break existing tests or the pty smoke's message expectations.

---

### Task 1: Help screen

**Files:** Create `src/help.rs`; modify `src/lib.rs`, `src/app.rs`, `src/app/tests.rs`, `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (layout line + dated log entry with honest done/unit-tested/pty/not-hand-verified/deferred and the real test count).

**Interfaces:**
- Produces: `Help { scroll }` with `new()`, `handle_key(KeyEvent) -> HelpAction {None, Back, ToggleGit, ToggleTree, Buffers, Next, Prev}`-style actions as needed (mirror `bufferlist.rs`), `render(width, height) -> Screen`, `set_message(String)`; `pub(crate) const HELP: &[(&str, &[(&str, &str)])]` (section title → (keys, description)).
- Consumes: `Screen`/`Span`/`Style`, `text::{clip, pad, window_start}` as `bufferlist.rs` does. `App` records the mode help was opened from (`help_from: Mode`).

- [ ] **Step 1: Failing tests** in `src/help.rs` (`#[cfg(test)] mod tests`) and `src/app/tests.rs`: table lists each required key name; render shows section titles and a key row; scroll clamps at both ends; 0..=6 x 0..=6 sweep; F1 from editor/git/tree/list shows help and `q`/Esc/F1 return to the origin; `?` opens it from the three views but types `?` in the editor; Ctrl-Q in help warns/quits per app rules; Ctrl-N leaves help to the next buffer; initial editor message is `F1: help` and disappears after one key.
- [ ] **Step 2:** run, see them FAIL for the right reason.
- [ ] **Step 3:** implement `help.rs`, wire `Mode::Help`, F1/`?` handling, `help_from`, hint message.
- [ ] **Step 4:** `cargo test` + clippy pass.
- [ ] **Step 5:** pty smoke scenario: F1 shows the Help screen with `Ctrl-S` and `Ctrl-Q` text, `q` returns to the editor text, session exits 0. Show it FAILING against the currently installed `~/.cargo/bin/ted` (stub-free: it lacks the feature), then passing on the new release build.
- [ ] **Step 6:** docs (README key table gets F1/`?` row and a "Help" section; CLAUDE.md) and commit `Add F1 help screen`.
