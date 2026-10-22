# Multiple Buffers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open several files at once in `ted`, switch between them, close them, and quit safely with unsaved changes in any of them.

**Architecture:** `App` owns `Vec<Editor>` plus `current` (each `Editor` already holds one buffer with its own cursor, scroll, highlight cache). Opening from the file tree or git view adds an editor (or switches to the one already showing that file) instead of refusing on a dirty buffer. Quit/close/switch keys live in `App`; `Editor` keeps editing, save and drawing. A new `Mode::Buffers` lists the open buffers.

**Tech Stack:** Rust, crossterm, ropey (existing). No new dependencies.

**Spec:** none written; the design is the user-approved call list in this plan's "Design calls" section (user chose the "proper fix" for opening files while the buffer is modified).

## Global Constraints

- TDD: failing test first. Logic in the library crate; `src/main.rs` stays a thin crossterm layer (should need no change, or only `app.editor()` call sites).
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- A missing file opens as an empty buffer only from the command line; the views refuse missing/non-regular/>64 MiB files as today (`App::load` checks stay). Never overwrite on a failed read.
- Every string drawn is sanitized (`text::sanitize`/`editor.rs` `clip`); the buffer list must sanitize file names too.
- macOS BSD shell portability in any script.
- Commit after each green test. Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls (user may veto)

- Keys (all work in every mode except where noted): Ctrl-N next buffer, Ctrl-P previous (wrap around), Ctrl-O buffer list, Ctrl-W close current, Ctrl-Q quit, Ctrl-S save current only.
- Status bar: ` name [+]` gains ` [2/3]` after the name only when more than one buffer is open (`[+]` stays as is).
- Opening a file already open (same canonical path, falling back to the joined path) switches to it. Otherwise it appends a new buffer and switches to it. If the only open buffer is unnamed, unmodified and empty, the new file replaces it.
- Ctrl-W on a modified buffer shows `Unsaved changes! Press Ctrl-W again to close without saving.`; any other key cancels; second Ctrl-W discards. Closing the last buffer leaves one empty unnamed buffer. After closing, the previous neighbour (index-1, or 0) becomes current.
- Ctrl-Q: if no buffer is modified, quit. Otherwise warn `Unsaved changes in N buffer(s)! Press Ctrl-Q again to quit without saving.`; any other key cancels; second Ctrl-Q quits.
- Buffer list: rows ` 1  name [+]` (current marked with `>`), Up/Down, Home/End, Enter switches and returns to the editor, `q`/Esc/Ctrl-O back. Ctrl-G / Ctrl-T from the list behave as in the editor.
- Ctrl-N/P/O/W in the git view and file tree: Ctrl-N/P/O work (leave the view, switch/open list); Ctrl-W is ignored there.

## Review Focus

- Open a file while the current buffer is modified: nothing lost, both buffers kept.
- Open the same file twice (also via `./x` vs `x`, and a symlink): one buffer, no duplicate.
- Close the last / only buffer; close while index is last; close while modified (arm, cancel by other key, confirm).
- Quit with a modified buffer that is not the current one: warns and counts it.
- A file name with control characters in the status bar and list is sanitized.
- Tiny terminal sizes (0..=6 x 0..=6) for the list and status with `[i/n]`: no panic.

---

### Task 1: Buffer set in App; open adds or switches; Ctrl-N/Ctrl-P; status tag

**Files:**
- Modify: `src/app.rs` (replace `editor: Editor` by `editors: Vec<Editor>`, `current: usize`; `editor()` returns the current one), `src/editor.rs` (status tag), `src/app/tests.rs`, `src/editor/tests.rs`, `src/main.rs` only if it breaks.

**Interfaces:**
- Produces: `Editor::set_tag(&mut self, tag: String)` — text inserted into the status line after the file name (and before ` [+]`? no: after `[+]`); empty means none. `App::buffer_count(&self) -> usize`, `App::current_index(&self) -> usize`.
- `App::open` no longer calls the dirty check; `App::load` loses its "Unsaved changes" refusal (delete that branch and its test, replace with the tests below). `Editor::load` is deleted if unused.

- [ ] **Step 1: Write failing tests** in `src/app/tests.rs` (use the existing helpers there for temp dirs and fake git):
  - `opening_while_modified_keeps_both_buffers`: edit buffer A (modified), open `b.txt` from tree → `buffer_count()==2`, `current_index()==1`, switching back (Ctrl-P) shows A still modified with its text.
  - `opening_open_file_switches`: open `b.txt` twice → count 2 (A + b), current is b; open A's own path → switches, count unchanged.
  - `opening_replaces_pristine_unnamed_buffer`: start with `Buffer::new()`, open a file → count 1.
  - `ctrl_n_and_ctrl_p_wrap`: with 3 buffers, Ctrl-N from last → 0; Ctrl-P from 0 → last. Each buffer keeps its own cursor row.
  - `status_shows_position_tag_only_with_many`: screen status contains `[2/3]` with 3 buffers, none with 1.
  - Editor test: `set_tag` text appears in `view().status`, sanitized, clipped to width.
- [ ] **Step 2:** Run `cargo test` — expect these to FAIL (missing methods / old refusal).
- [ ] **Step 3:** Implement. Path identity helper `fn same_file(a: &Path, b: &Path) -> bool` in `app.rs`: compare `std::fs::canonicalize` of both when both succeed, else compare the paths. Before each `screen()` call App sets the current editor's tag (`""` or `[i/n]`). Ctrl-N/P handled in `App::handle_key` before mode dispatch; in a view they also set `mode = Editor`.
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets` pass.
- [ ] **Step 5:** Commit `Hold several buffers; opening a file adds or switches to it`.

### Task 2: Ctrl-W close

**Files:** Modify `src/app.rs`, `src/app/tests.rs`.

**Interfaces:** Consumes Task 1's `editors`/`current`. Produces `App::close_current`, private; a `close_armed: bool` field cleared by any other key (same pattern as the quit warning) and by mode changes.

- [ ] **Step 1: Failing tests:** close clean buffer of 3 → count 2 and current is previous neighbour; close buffer 0 → current 0; close only buffer → count 1, unnamed, empty; close modified → warning message in editor view, count unchanged; arm then another key → disarmed (third Ctrl-W warns again); arm then second Ctrl-W → closed and its edits gone; Ctrl-W in the file tree is ignored.
- [ ] **Step 2:** run, see FAIL. **Step 3:** implement; the warning text is in the Design calls; show it through the current editor's message (`Editor::set_message(String)` — add if missing) and `set_view_message` when in a view. **Step 4:** tests + clippy pass. **Step 5:** commit `Ctrl-W closes the current buffer`.

### Task 3: Quit checks every buffer

**Files:** Modify `src/app.rs`, `src/editor.rs` (remove Ctrl-Q handling and `quit_armed`/`cancel_quit` if now unused), `src/app/tests.rs`, `src/editor/tests.rs`.

**Interfaces:** `App` owns `quit_armed: bool`; Ctrl-Q is intercepted in `App::handle_key` in every mode. The warning appears in the current editor's message (editor mode) or via `set_view_message`.

- [ ] **Step 1: Failing tests:** no modified buffers → `Flow::Quit` at once; modified non-current buffer → warning containing `1 buffer` and no quit; two modified → `2 buffers`; second Ctrl-Q quits; any other key (including Ctrl-N, Ctrl-G) between cancels; the previous editor quit tests are ported to `App` (delete them from `editor/tests.rs`).
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** `cargo test` + clippy. **Step 5:** commit `Ctrl-Q warns about unsaved changes in any buffer`.

### Task 4: Buffer list (Ctrl-O), docs, pty smoke

**Files:** Create `src/bufferlist.rs` (pure state: `BufferList { selected }`, `handle_key -> ListAction {None, Back, Switch(usize), ToggleGit, ToggleTree}`, `render(entries: &[(String, bool)], current, width, height) -> Screen`); modify `src/lib.rs`, `src/app.rs` (`Mode::Buffers`), tests in `src/bufferlist.rs` (`#[cfg(test)] mod tests`) and `src/app/tests.rs`, `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (layout line + dated log entry listing done / not hand-verified / deferred).

**Interfaces:** Consumes `Screen`/`Span`/`Style` and `text::{sanitize, clip, pad, styled, window_start}` as the file tree does. Entry label is the file name (or `[No Name]`), modified adds ` [+]`.

- [ ] **Step 1: Failing tests:** rendering (numbers, `>` on current, `[+]`, sanitized hostile name, 0..=6 x 0..=6 sweep without panic), Up/Down clamp, Home/End, Enter → `Switch(i)`, `q`/Esc/Ctrl-O → `Back`; App: Ctrl-O shows the list, Enter switches and returns to editor, list reflects a buffer opened from the tree, selection starts on the current buffer.
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** tests + clippy pass.
- [ ] **Step 5:** pty smoke: add a scenario (hermetic dir with `a.txt`, `b.txt`): open both via Ctrl-T/Enter, type in the first, assert status shows `[2/2]`, Ctrl-O lists both names, Enter returns, Ctrl-W warns on the modified one, Ctrl-Q warns with `1 buffer`, then quits with exit 0. Show it failing with the new keys stubbed out (stub-and-restore), then passing: `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`.
- [ ] **Step 6:** Docs; commit `Buffer list (Ctrl-O), docs and pty smoke for multiple buffers`.
