# Command Prompt Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ctrl-X opens a one-line command prompt (`Command: _`) in the message row; you type a command name (Tab completes), press Enter, and it runs. First real new command: `saveas` for unnamed buffers.

**Architecture:** A pure `src/command.rs` (names, prefix matching, completion, parsing into a `Command` enum). The prompt itself is a third `PromptKind::Command` in `Editor`, so routing (every key goes to an open prompt, Ctrl-Q cancels it), rendering, tiny-size clipping and sanitizing are reused. On Enter the editor stores the typed text; `App` takes it (`Editor::take_command`) and runs it, because most commands act on buffers, views and the line number setting.

**Tech Stack:** Rust, existing crates only.

**Spec:** none; design calls below (user may veto).

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` untouched.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Never overwrite on a failed read; `saveas` never overwrites an existing file. Every drawn string stays sanitized (the prompt text is user-typed and goes through `clip`).
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls

- Key: Ctrl-X, editor mode only (like Ctrl-F/Ctrl-L). Esc cancels. Modeless editor stays; no `:` key.
- Commands (canonical names, completed by Tab): `save`, `saveas <path>`, `goto <n>`, `lines [on|off]`, `next`, `prev`, `close`, `buffers`, `tree`, `git`, `help`, `quit`. Aliases (parse only, not completed): `w` = save, `q` = quit, `wq` = save then quit (quits only if the save left the buffer unmodified).
- Matching: exact name or alias wins; otherwise a unique prefix runs (`buf` = buffers); several matches show `Ambiguous: save, saveas` and nothing runs; none shows `Unknown command: foo`. Names are case-insensitive; arguments are not.
- Tab (command word only, no space typed yet): completes to the longest common prefix of the matching names, plus a trailing space when the result is one full name that takes an argument (`goto`, `saveas`, `lines`). With a space typed Tab does nothing.
- Prompt line: `Command: sa_` followed, while the word has matches and no space is typed, by `  save saveas` (the matching names), clipped like any message. After Enter the prompt closes; results appear as the normal message (`Saved N lines`, errors, `Unknown command: x`).
- `goto` needs a positive integer (`Usage: goto <line>` otherwise) and behaves like Ctrl-L. `lines` with no argument toggles the same setting as F2 (all buffers). `close` and `quit` use the existing Ctrl-W / Ctrl-Q two-press rules: the warning is armed by the command and the next key must be Ctrl-W / Ctrl-Q. `next`/`prev`/`buffers`/`tree`/`git`/`help` act like Ctrl-N/Ctrl-P/Ctrl-O/Ctrl-T/Ctrl-G/F1.
- `saveas <path>`: relative paths resolve against `App`'s location directory; refuses (message, nothing changes) if the target exists (`Exists: <name>`), if another open buffer already has that file (`Already open: <name>`), or the path is empty (`Usage: saveas <path>`); a failed write keeps the buffer's old path and modified state and shows `Save failed: <error>`. On success the buffer takes the path (language/highlight follow the new extension) and is unmodified.

## Review Focus

- Prompt keys: Esc cancels, Backspace past empty keeps the prompt open, Ctrl-Q cancels the prompt then follows the quit rules, F1/Ctrl-N/Ctrl-G inside the prompt are text or ignored, not global actions.
- Hostile text typed or pasted into the prompt or a `saveas` path (control chars, ESC) is sanitized when drawn; `saveas` with `..`, a directory, an existing file, a missing directory, an empty name.
- `close`/`quit` via command on a modified buffer: warning shows, next Ctrl-W / Ctrl-Q completes, any other key withdraws it.
- `wq` on an unnamed or unsaveable buffer does not quit and does not lose data.
- Tiny terminal sizes 0..=6 x 0..=6 with the prompt open (editor and App): no panic.

---

### Task 1: `command.rs` and the Command prompt in `Editor`

**Files:**
- Create: `src/command.rs` (+ `pub mod command;` in `src/lib.rs`), `src/editor/command_tests.rs` (add `#[cfg(test)] mod command_tests;` in `editor.rs`).
- Modify: `src/editor.rs`.

**Interfaces:**
- Produces in `src/command.rs`:
  - `pub const NAMES: [&str; 11]` — `save goto lines next prev close buffers tree git help quit` (saveas is added in Task 3; keep NAMES an array whose length the tests do not hard-code beyond using `NAMES`).
  - `pub enum Command { Save, Goto(usize), Lines(Option<bool>), Next, Prev, Close, Buffers, Tree, Git, Help, Quit, WriteQuit }` (derive Debug, Clone, PartialEq, Eq).
  - `pub fn parse(input: &str) -> Result<Command, String>` — trims; first word case-insensitive; error strings exactly `Unknown command: <word>`, `Ambiguous: <n1>, <n2>` (names in NAMES order), `Usage: goto <line>`, `Usage: lines [on|off]`; empty input is `Err(String::new())` (nothing to run, no message). Extra arguments to commands that take none are an error `Usage: <name>`.
  - `pub fn matches(word: &str) -> Vec<&'static str>` — names starting with the lowercase word, in NAMES order.
  - `pub fn complete(word: &str) -> String` — Tab result as described in Design calls (returns the word unchanged when nothing matches).
- Produces in `Editor`: `PromptKind::Command`; Ctrl-X opens it; `pub fn take_command(&mut self) -> Option<String>` returns (and clears) the text typed before an accepted Enter (non-empty after trim only); Tab handling; `prompt_line` shows `Command: {text}_` plus the match list suffix. Esc cancels without a stored command.

- [ ] **Step 1: Failing tests.** `command.rs`: parse each command and the aliases (`w`, `q`, `wq`); `PARSE goto 12` → `Goto(12)`; `goto`, `goto 0`, `goto x` → usage error; `lines`, `lines on`, `lines off`, `lines maybe`; prefix `buf` → `Buffers`; `s` → `Ambiguous: save` ... (with only `save` starting with `s` in this task it is `Save`; the Ambiguous case is tested with `g` → `Ambiguous: goto, git`); `next now` → `Usage: next`; unknown word; empty; `matches`/`complete` (`go` → `goto `, `g` → `g` (common prefix of goto, git), `qu` → `quit`, `zzz` → `zzz`, uppercase input). Editor: Ctrl-X opens the prompt showing `Command: _`; typing, Backspace, Esc (no command taken); Enter stores the text and `take_command()` returns it once; Enter on empty/blank stores nothing; Tab completes in the prompt text; the suffix lists matches while typing and disappears after a space; control chars typed in the prompt are drawn as `?`; Ctrl-F/Ctrl-L inside the prompt are not actions; 0..=6 x 0..=6 sweep with the prompt open.
- [ ] **Step 2:** `cargo test` — confirm failures are for the right reason (missing module/variant).
- [ ] **Step 3:** Implement `command.rs`, then the `Editor` prompt kind (open on Ctrl-X in `handle_key`, `prompt_key` arms for Command: char/Backspace/Tab/Enter/Esc, `prompt_line`).
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** Commit `Command prompt: command names, parsing, completion and the Ctrl-X prompt`.

### Task 2: `App` runs the commands

**Files:** Modify `src/app.rs`, `src/editor.rs` (make `save` public, add `pub fn go_to_line(&mut self, n: usize)` delegating to the buffer), tests in `src/app/tests/command.rs` (add the module the same way as `search.rs`).

**Interfaces:**
- Consumes `command::{parse, Command}` and `Editor::take_command`.
- Produces `App::run_command(&mut self, text: &str, now: SystemTime) -> Flow`, called from `handle_key` right after the editor handled a key in editor mode (`if let Some(text) = editor.take_command()`), so the key that pressed Enter also arms warnings correctly. A parse error becomes the current editor's message (empty error: no message).

- [ ] **Step 1: Failing tests (App level, using the existing helpers):** each command does what its key does (`goto 3` moves the cursor; `lines` toggles and `lines off`/`lines on` set; `next`/`prev` cycle; `buffers`, `tree`, `git`, `help` switch mode; `close` on a clean buffer closes, on a modified one shows the Ctrl-W warning, and a following Ctrl-W closes while another key withdraws it; `quit` clean → `Flow::Quit`, with a modified buffer in another tab shows the `1 buffer` warning and Ctrl-Q then quits; `save` saves (`Saved N lines`); `wq` saves and quits, and on an unnamed buffer shows the no-file-name message and does not quit); errors: `foo` → `Unknown command: foo` in the screen's message row; Ctrl-X then Esc leaves everything unchanged; Ctrl-X ignored in the git view, tree, list and help; Ctrl-Q with the prompt open cancels it and follows the quit rules; sweep 0..=6 x 0..=6 with the prompt open.
- [ ] **Step 2:** run, see FAIL. **Step 3:** implement. **Step 4:** `cargo test && cargo clippy --all-targets`. **Step 5:** commit `Command prompt: run commands from App`.

### Task 3: `saveas`, help, docs, pty smoke

**Files:** Modify `src/buffer.rs` (`save_as`), `src/command.rs` (`saveas` name + `Command::SaveAs(String)` + parse/usage `Usage: saveas <path>`, `NAMES` grows to 12, completion adds a trailing space), `src/app.rs` (execution, path checks), `src/help.rs` (table row + pinned-key test), `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (layout line for `command.rs`, dated log entry: done / not hand-verified / deferred), tests next to each.

**Interfaces:** Produces `Buffer::save_as(&mut self, path: PathBuf) -> io::Result<()>`: atomic write to `path` using the existing save routine; only on success does the buffer take the new path and clear its modified flag; on failure path and modified state are unchanged. `App` resolves a relative path against its location, rejects an existing target and a path already open in another buffer (same-file check as the buffer-opening code), then calls the editor's `save_as` and shows `Saved N lines` / `Save failed: <e>`.

- [ ] **Step 1: Failing tests:** `Buffer::save_as` writes the bytes, sets the path, clears modified; a failed write (missing directory) leaves path/modified untouched and creates no file; App: `saveas new.txt` on an unnamed buffer creates the file under the location and the status shows its name (the `.rs` extension turns highlighting on); existing target → `Exists: <name>` and the file is untouched; path open in another buffer → `Already open: <name>`; empty → usage; hostile name drawn sanitized; `wq` on an unnamed buffer still does not quit; help table lists Ctrl-X and the pinned key list test is updated (Ctrl-S/Ctrl-Q stay on the first 80x24 screen).
- [ ] **Step 2:** run, FAIL. **Step 3:** implement. **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** pty smoke scenario (hermetic temp dir, start `ted` with no file): Ctrl-X, `goto 3`, Enter on a 12-line file case is optional; required: start on a file with 12 lines, Ctrl-X + `lines` + Enter hides the gutter (strip style escapes), Ctrl-X + `lines` + Enter restores it; Ctrl-X + `zzz` + Enter shows `Unknown command: zzz`; then in a second run with no file: type text, Ctrl-X + `saveas out.txt` + Enter, the status shows `out.txt`, the file exists with the typed text, Ctrl-Q exits 0. Show the new checks failing against the previously installed `~/.cargo/bin/ted`, then passing on the release build.
- [ ] **Step 6:** Docs; commit `Command prompt: saveas, help, docs, pty smoke`.
