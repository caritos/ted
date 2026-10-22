# ted

A small terminal text editor in Rust, for my own use. Goal: it just works, no configuration.

## Layout
- `src/buffer.rs` - rope-backed text + cursor + file load/atomic save. No terminal code.
- `src/editor.rs` - key handling, scrolling, status/message, quit confirmation. `Editor::view()` returns a pure `View`.
- `src/git.rs` - runs the `git` CLI (read-only) behind a `Git` trait: status, diff, log, baseline. No terminal code.
- `src/gitview.rs` - git view state machine: file list, selection by path, diff scroll, commit picker, refresh.
- `src/text.rs` - shared text helpers (width fitting, truncation) used by the views.
- `src/filetree.rs` - read-only file tree state (expansion, selection, refresh, rendering to rows). No terminal code.
- `src/app.rs` - owns the `Editor`, the optional git view and file tree (`Mode` enum); Ctrl-G / Ctrl-T / `q` switch modes.
- `src/screen.rs` - plain `Screen` value (cells + styles) that both modes render to.
- `src/main.rs` - thin crossterm layer only: raw mode, read keys, draw the `View`. Keep logic out of it.
- `scripts/pty_smoke.py` - drives the release binary through a pty.

## Commands
- Fast tier: `cargo test`
- Lint: `cargo clippy --all-targets`
- Real-terminal check: `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`
- Run: `cargo run -- <file>`; install: `cargo install --path .`
- Rust comes from rustup via Homebrew; if `cargo` is missing, `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.

## Rules
- TDD: failing test first. Logic goes in the library (`buffer`, `editor`, `git`, `gitview`, `app`) so it is testable without a terminal.
- A missing file opens as an empty buffer; any other read error (incl. non-UTF-8) is an error. Never overwrite on a failed read.

## Log
### 2026-10-22 (file tree)
- Done (unit-tested): `FileTree` state machine (expand/collapse, selection, Enter/Left/Right, PageUp/Down, Home/End, unreadable dirs, refresh), rendering with `Style::Dir`, `App` Mode enum with Ctrl-T / Ctrl-G cross-switching.
- Done (pty smoke): Ctrl-T lists entries and a dotfile, Right expands `sub/`, Enter opens `top.txt`, Ctrl-T twice returns to the editor, Ctrl-Q exits 0. Seen failing with the Ctrl-T handler stubbed out.
- Not yet verified by hand: colors, resize, a very large directory, symlinked directories in real use.
- Deferred: sidebar next to the editor, git status marks in the tree, create/rename/delete, filtering/search, hidden-file toggle.
### 2026-10-22
- Done (unit-tested + pty smoke): open/edit/save file, arrows, Backspace/Delete/Enter/Tab(4 spaces), Ctrl-S, Ctrl-Q with unsaved-changes confirm, scrolling, status bar.
- Confirmed by hand in a real terminal (2026-10-22): save, quit, add text, delete text.
- Not yet verified by hand: look and feel, resize, Option/Alt keys, unsaved-changes quit warning.
- Deferred: Home/End, PageUp/Down, undo, search, save-as prompt for unnamed buffers, wide-char (CJK/emoji) cursor alignment, syntax highlighting.
- History: an earlier SwiftUI Mac app attempt lives at git tag `swift-mac-app`.
- Environment: Homebrew rustup is keg-only; `~/src/config/zsh/zshenv` now adds it and `~/.cargo/bin` to PATH (change is uncommitted in the config repo). See README.md for user-facing docs.
### 2026-10-22 (git changes view)
- Done, unit-tested: `ted <dir>` opens an empty buffer located in that directory; git view with file list + diff, Up/Down, PageUp/PageDown, Enter opens a file, `r` resets the baseline, `b` recent-commits picker, `q`/Esc back, "not a git repository" for a non-repo. Done, pty smoke (hermetic git env): Ctrl-G, `q`, file list (tracked + untracked), a `+` diff line, dirty-buffer round trip, non-repo error. Up/Down, PageUp/PageDown, Enter, `r`, `b` and Esc are unit-tested only. Design: `docs/superpowers/specs/2026-10-22-changes-viewer-design.md`.
- Not yet verified by hand: the live refresh while Claude edits files, resize inside the git view, colors on a real terminal, a very large repo.
- Deferred: wide-char alignment in the git view, mouse, word diffs, a configurable refresh rate.
