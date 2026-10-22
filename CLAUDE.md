# ted

A small terminal text editor in Rust, for my own use. Goal: it just works, no configuration.

## Layout
- `src/buffer.rs` - rope-backed text + cursor + file load/atomic save. No terminal code.
- `src/editor.rs` - key handling, scrolling, status/message, quit confirmation. `Editor::view()` returns a pure `View`.
- `src/main.rs` - thin crossterm layer only: raw mode, read keys, draw the `View`. Keep logic out of it.
- `scripts/pty_smoke.py` - drives the release binary through a pty.

## Commands
- Fast tier: `cargo test`
- Lint: `cargo clippy --all-targets`
- Real-terminal check: `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`
- Run: `cargo run -- <file>`; install: `cargo install --path .`
- Rust comes from rustup via Homebrew; if `cargo` is missing, `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.

## Rules
- TDD: failing test first. Logic goes in `buffer`/`editor` so it is testable without a terminal.
- A missing file opens as an empty buffer; any other read error (incl. non-UTF-8) is an error. Never overwrite on a failed read.

## Log
### 2026-10-22
- Done (unit-tested + pty smoke): open/edit/save file, arrows, Backspace/Delete/Enter/Tab(4 spaces), Ctrl-S, Ctrl-Q with unsaved-changes confirm, scrolling, status bar.
- Not yet verified by hand in a real terminal app (Terminal/iTerm): look and feel, resize, Option/Alt keys.
- Deferred: Home/End, PageUp/Down, undo, search, save-as prompt for unnamed buffers, wide-char (CJK/emoji) cursor alignment, syntax highlighting.
- History: an earlier SwiftUI Mac app attempt lives at git tag `swift-mac-app`.
