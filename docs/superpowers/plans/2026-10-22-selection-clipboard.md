# Selection and Clipboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Select text with Shift+movement keys (or Ctrl-A for everything), copy / cut / paste it with the system clipboard, replace or delete a selection by typing, and paste as one undo step (bracketed paste) instead of a flood of keystrokes.

**Architecture:** `Buffer` gets an optional selection anchor (the cursor is the other end) and range-based edits that go through the existing undo `Edit` (one step each, so undo/redo and the modified flag keep working). `Editor` maps keys and draws selected cells with a new `Style::Selected`. A `Clipboard` trait in a new `src/clipboard.rs` (system implementation + in-memory fake) is owned by `App`, which handles copy/cut/paste and the terminal's bracketed-paste event (`App::paste`). `main.rs` only enables bracketed paste, forwards `Event::Paste`, and draws `Style::Selected` reversed.

**Tech Stack:** Rust, crossterm, ropey; `pbcopy`/`pbpaste` (macOS) via `std::process::Command`, no new crates.

**Spec:** none; design calls below (user may veto).

## Global Constraints

- TDD: failing test first. Logic in the library crate; `src/main.rs` stays a thin crossterm layer.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted` (can take several minutes: use a generous timeout). If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Cursor columns stay char-based and every re-placement goes through `place_cursor` (lone `\r`, `\r\n`, see issue #2). Every changed row is `mark_dirty`ed for the highlight cache. All drawn text is still sanitized one-for-one (`draw_cells`/`clip`); pasted text is stored raw in the buffer but never reaches the terminal unsanitized.
- Undo: every selection edit and every paste is exactly one undo step (`Edit` with `removed` and/or `inserted`), never merged with neighbours; cursor `before`/`after` recorded; `modified` follows the history rules already in `Buffer`.
- The system clipboard is best effort: failures never lose text or panic (fall back to the in-memory register and say so in the message only when nothing at all could be read).
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls

- Select: Shift+Left/Right/Up/Down/Home/End/PageUp/PageDown extend the selection from where it started (anchor stays, cursor moves like the unshifted key, goal column kept). Ctrl-A selects everything (cursor at the end). Any unshifted movement key, a search jump, go-to-line, undo/redo, save, or Esc clears the selection (Esc only when no prompt is open). Selection is per buffer and is not kept across a buffer switch (cleared on switch).
- Typing a character, Tab, Enter, Backspace or Delete with a selection: the selection is deleted first (Backspace/Delete just delete it); typing/Tab/Enter then insert as usual, all as one undo step with the deletion. A replace-by-typing is NOT merged into the following typing run.
- Copy: Ctrl-C. Cut: Ctrl-K (Ctrl-X is the command prompt). Paste: Ctrl-V. With no selection, copy and cut do nothing except the message `Nothing selected`. Messages on success: none (copy shows `Copied N chars`, cut `Cut N chars`; paste shows nothing).
- Clipboard: `Clipboard` trait `get() -> Option<String>`, `set(&str) -> bool`. System implementation: macOS `pbcopy` / `pbpaste` when present, otherwise `wl-copy`/`wl-paste` or `xclip -selection clipboard`, otherwise unavailable. `App` also keeps an internal register: copy/cut set both; paste prefers the system clipboard text and falls back to the register. Paste of empty text does nothing.
- Bracketed paste: `main.rs` enables it on entry and disables it on exit (and in the panic/restore path that already restores the terminal); `Event::Paste(String)` goes to `App::paste(&str)`: in the editor it inserts the whole text as one undo step (CRLF and lone CR normalised to `\n`, tabs kept); in an open Find/Goto/Command prompt it inserts the text with line breaks removed (Goto keeps digits only); in views/help it is ignored. This also fixes "a pasted newline runs the command prompt".
- Drawing: selected cells use `Style::Selected` (reverse video in `main.rs`) and take priority over syntax styles; selection that spans lines highlights each line's selected cells (an empty-line break is not drawn); the gutter is never selected; tabs are selected cell by cell; a selection with the cursor at either end looks the same.
- Status/Quit/modified behaviour unchanged. Out of scope: mouse, rectangular selection, word-wise selection (Ctrl-Shift-arrows), selection in the views, copy from the git view, clipboard history, OSC 52.

## Review Focus

- Selection across line breaks incl. lone `\r` / `\r\n`, tabs, an empty selection (anchor == cursor), selection at end of file, selecting then undo/redo (text, cursor, modified flag, selection cleared).
- Typing over a selection, Backspace/Delete, Enter, Tab; Ctrl-K then undo restores text and clipboard keeps the text.
- Paste of: multi-line text, text ending without newline, CRLF text, tabs, control chars/ESC (drawn as `?`, saved bytes keep them as pasted), a 5 MB paste (must stay fast: one rope insert, no per-char loop with history merging), pasting over a selection (one undo step), pasting into prompts (no command runs, no newline in the prompt).
- Clipboard failures: missing `pbpaste`, command exits non-zero, non-UTF-8 output (treat as unavailable, fall back to the register), invalid selections after the buffer changed.
- Tiny terminal sizes 0..=16 x 0..=6 with a selection and gutter on/off: no panic; shifted keys in views/help/buffer list do nothing new.

---

### Task 1: Selection and range edits in `Buffer`

**Files:** Modify `src/buffer.rs` (selection state, `Edit` use), tests in a new `src/buffer/selection_tests.rs` (wire like `undo_tests.rs`).

**Interfaces:**
- Produces on `Buffer`: `selection(&self) -> Option<((usize, usize), (usize, usize))>` (start, end as (row, col), start < end in document order, `None` when empty/unset); `select_all(&mut self)`; `clear_selection(&mut self)`; `selected_text(&self) -> Option<String>`; `delete_selection(&mut self) -> bool` (one undo step; cursor to the start; false if nothing selected); `insert_str(&mut self, text: &str)` (inserts at the cursor, replacing the selection if any, as ONE undo step including the deletion; text normalised only by the caller; cursor ends after the inserted text; marks dirty from the first affected row); and `extend` variants of the moves: `move_left/right/up/down/home/end/up_by/down_by` each get a `select: bool` counterpart via a new `fn extend_selection(&mut self, f: impl FnOnce(&mut Buffer))` that sets the anchor to the current cursor if none, runs `f`, and keeps the anchor (clearing it if the cursor returns to it). The existing unshifted moves, `set_cursor`, `go_to_line`, `undo`, `redo`, `save` and `save_as` clear the selection.
- Existing `insert_char`, `insert_newline`, `backspace`, `delete` replace/delete the selection first when one exists (one step with the deletion; Backspace/Delete on a selection only delete it).

- [ ] **Step 1: Failing tests:** selection after extend moves in all directions; select_all; selected_text across lines (`\n`, `\r\n`, lone `\r`) and multibyte chars; delete_selection restores via one undo (text, cursor, modified flag) and redo repeats it; insert_str with and without selection is one undo step and the modified flag/dirty row are right; typing over a selection is one step with the deletion and the next typed char starts a new merge run; Backspace/Delete on a selection; moves/set_cursor/go_to_line/undo/redo/save clear it; extending back to the anchor leaves no selection; randomized test against a simple reference (random edits, selections, deletes, inserts, undo/redo; text always equals the reference; cursor valid; undo-all restores the original); a 5 MB `insert_str` completes in well under a second in a debug test run.
- [ ] **Step 2:** `cargo test` — confirm failures are for the right reason.
- [ ] **Step 3:** Implement. Reuse the undo `Edit` (it already carries `removed` and `inserted`); no per-char loops for ranges.
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** Commit `Buffer: selection and range edits`.

### Task 2: Selection in `Editor` (keys and drawing)

**Files:** Modify `src/editor.rs` (keys, `draw_cells`, `styles` combination), `src/screen.rs` (`Style::Selected`), `src/main.rs` (draw `Style::Selected` with reverse video; the only terminal change here), tests in `src/editor/selection_tests.rs`, `src/app/tests/` for routing.

**Interfaces:** Consumes Task 1. Produces `Style::Selected`; `Editor::handle_key` handles SHIFT+Left/Right/Up/Down/Home/End/PageUp/PageDown, Ctrl-A, Esc (clears when no prompt), and typed keys over a selection; selection cleared by every unshifted move key.

- [ ] **Step 1: Failing tests:** Shift+Right selects one char and the view shows `Style::Selected` on exactly that cell; a multi-line selection styles each line's selected cells; selection starting/ending off-screen (scrolled) styles the visible part only; tabs in the selection are styled cell by cell; selection wins over syntax colors on a `.rs` file; the gutter cells are never `Selected`; with horizontal scroll, columns line up; Ctrl-A then typing replaces everything and undo restores it; Backspace deletes the selection; unshifted Right clears it; Esc clears it but still cancels an open prompt first; the status line is unchanged; in the git view, tree, buffer list and help, shifted keys do nothing new; sweep 0..=16 x 0..=6 with a selection and gutter on/off, no panic; switching buffers clears the selection.
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** Commit `Editor: select with Shift+movement and Ctrl-A, draw the selection`.

### Task 3: Clipboard, copy/cut/paste, bracketed paste, docs, pty

**Files:** Create `src/clipboard.rs` (+ `pub mod clipboard;` in `src/lib.rs`); modify `src/app.rs` (clipboard + register, Ctrl-C / Ctrl-K / Ctrl-V in editor mode only and not in prompts, `App::paste`), `src/main.rs` (enable/disable bracketed paste, forward `Event::Paste`), `src/command.rs` (commands `copy`, `cut`, `paste`, `selectall`: names, `Command` variants, parse; check prefix ambiguity changes against existing tests), `src/help.rs` (rows; Ctrl-S/Ctrl-Q stay on the first 80x24 screen; no row over 80 columns), `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (layout, dated log entry: done / not hand-verified / deferred), tests in `src/clipboard.rs` and `src/app/tests/clipboard.rs`.

**Interfaces:** `pub trait Clipboard { fn get(&mut self) -> Option<String>; fn set(&mut self, text: &str) -> bool; }`, `pub struct SystemClipboard` (the real one; every spawn has a timeout of 2 s and never blocks the UI longer), `pub struct MemoryClipboard` (tests), `App::new` keeps its signature and defaults to `SystemClipboard`; add `App::with_clipboard(...)` (or a builder) for tests. `App::paste(&mut self, text: &str)`.

- [ ] **Step 1: Failing tests:** App with `MemoryClipboard`: copy puts the selection in the clipboard and register and leaves the text; cut removes it (one undo step restores); paste inserts clipboard text at the cursor (replacing a selection, one undo step); paste falls back to the register when the clipboard is empty/unavailable; copy/cut with no selection show `Nothing selected`; `copy`/`cut`/`paste`/`selectall` commands; `App::paste` multi-line text, CRLF and lone CR normalised, one undo step, a 5 MB paste fast; paste into a Find prompt (no newline, search runs), into Goto (digits only), into the Command prompt (no command runs); paste ignored in views and help; Ctrl-C/Ctrl-K/Ctrl-V ignored in prompts and views; hostile pasted text drawn sanitized and saved bytes equal the pasted bytes; `SystemClipboard` parsing is exercised only through a small seam (the command runner), with tests for: command missing, non-zero exit, non-UTF-8 output, timeout.
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** pty smoke: a file with a few lines; Shift+Right x3 (send `ESC[1;2C`) draws a reversed span (`\x1b[7m`) over the first chars; type `X` replaces them; Ctrl-Z restores; Ctrl-A, Ctrl-K cuts all (buffer empty, status `[+]`), Ctrl-V pastes it back (use the in-memory register fallback: set `PATH` in the spawned environment to an empty directory so no system clipboard tool is found); a bracketed paste (`ESC[200~` + multi-line text + `ESC[201~`) lands as text and a single Ctrl-Z removes the whole paste; a bracketed paste containing a newline into the Ctrl-X command prompt does not run it. Show the new checks failing against `~/.cargo/bin/ted`, then passing on the release build.
- [ ] **Step 6:** Docs; commit `Clipboard: copy, cut, paste and bracketed paste`.
