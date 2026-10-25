# ted: project state (written 2026-10-25, paused)

Read this first when resuming. Layout, rules and commands are in `CLAUDE.md`; user-facing docs are in `README.md`; the dated per-feature log (what was tested, what was not) is at the bottom of `CLAUDE.md`.

## Status
- Paused on purpose: the owner wants to use ted for real work for a while (or fall back to vim) before building more. Nothing is half-done. `main` is merged, pushed (`origin/main`) and installed (`~/.cargo/bin/ted` via `cargo install --path .`).
- 973 unit tests, `cargo clippy --all-targets` clean, `scripts/pty_smoke.py` 0 failures on the release build.
- About 7,300 lines of Rust in `src/` (crossterm + ropey). Logic lives in library modules; `main.rs` is the only terminal code.

## What it can do
Editing: open/save (atomic), undo/redo (per buffer), selection and clipboard (pbcopy/pbpaste, wl-clipboard, xclip, bracketed paste), word moves and deletes, auto-indent, smart indent (deeper after `{ ( [`, `{|}` split, dedent on typing `} ) ]`), block indent/dedent (Tab / Shift-Tab on 2+ rows), line operations (duplicate, delete, move, comment toggle by file type), jump to matching bracket (Ctrl-]), tab stops, line-number gutter, Rust syntax highlighting.
Search: Ctrl-F incremental find with match underline, F3 next, Ctrl-R replace all, `qreplace` command (y/n/a/q), Ctrl-L go to line, one search term shared by all buffers.
Navigation: several buffers (Ctrl-N/P, Ctrl-O list, Ctrl-W close), file tree (Ctrl-T), fuzzy open by name (Ctrl-E, recent files first), find in files (F4), git changes view (Ctrl-G, read-only: status, diff, recent-commit baseline), Ctrl-X command prompt with Tab completion, F1 help.
The full key list is the help screen (`src/help.rs`) and the README tables.

## Not done (known gaps)
- Mouse (issue #5, low priority). Regex search/replace. Wide characters (CJK/emoji) alignment. Syntax highlighting for languages other than Rust. Persisted settings (line numbers, recents, search history). Auto-closing brackets. Soft wrap. Multiple cursors. Split windows. LSP.
- Smaller deferred ideas are listed per feature in the CLAUDE.md log ("Deferred:" lines): relative line numbers, grep off the UI thread / regex / match highlight, join lines, block comments, Python `:` indent, a smart-indent off switch.

## Never verified by hand in a real terminal
Everything since the first day was checked by unit tests and the pty smoke only (the smoke drives the release binary through a pty, so escape sequences are tested, rendering on a real theme is not). Confirmed by hand once (2026-10-22): save, quit, add and delete text. Highest-risk items, in order:
1. Keys that depend on the terminal: Alt-Up/Down, Alt-K, Alt-/ and Option+arrows need "Option as Meta"; Ctrl-/ (arrives as Ctrl-7 or Ctrl-_), Ctrl-] (0x1d), Shift-Tab, F-keys (F1-F4), Ctrl-E and Ctrl-R, all of them also inside tmux and over ssh. Fallback for the line operations: the Ctrl-X commands `duplicate deleteline moveup movedown`.
2. Smart indent changes what Enter does while typing, in every file type; it may annoy in plain text.
3. Find in files is synchronous and reads every listed file: the screen freezes while it runs (never measured on a big repo; cap is 2,000 hits, files over 2 MiB skipped).
4. The finder ranks with a full scan per keystroke (cap 100,000 files); recents are matched by path and may not match through symlinks.
5. Look and feel: bold vs dim line numbers, underline on search matches, reverse-video selection, colours of the highlighting.
The checklist for this is GitHub issue #4.

## Open GitHub issues
- #5 mouse support (low priority) - #4 hand-verify checklist (only the owner can do it) - #3 hardening follow-ups (minor) - #1 IntelliJ-inspired wishlist.

## How to resume
1. `cd ~/src/ted`, `git pull`, `cargo test` (rustup is keg-only: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` if `cargo` is missing).
2. Use the installed `ted` for real edits; note friction in issue #4 or as new issues. Prefer fixing what actually hurt over the deferred list.
3. Work flow (from the owner's global rules): TDD (failing test first), logic in library modules, a worktree per feature (`EnterWorktree`), small commits, full `cargo test` + `cargo clippy --all-targets` + `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted` before merging, merge to `main` locally and re-run the suite, push main (allowed without asking in this repo), `cargo install --path .`, and update the CLAUDE.md log (Done / Not yet verified by hand / Deferred).
4. Gotchas: in this sandbox, shell commands mixing inline python/heredocs with `git` or `worktree` were refused in worktree sessions (write a script file or use the Edit tool, run git as separate calls); macOS BSD `sed -i ''`; commit trailers are `Co-Authored-By` plus `Claude-Session` lines.

## Design rules worth remembering
- A missing file opens as an empty buffer; any other read error is an error; never overwrite on a failed read.
- Every string drawn is sanitized (control, bidi and zero-width chars become `?`); only `editor.rs` `clip` / `draw_cells` build editor `View` strings.
- Edits that touch several places (replace, line operations, indent, paste) are one rope edit and one undo step (`replace_range_to`, `insert_str`).
- Old specs and plans are in `docs/superpowers/`; an earlier SwiftUI Mac attempt lives at git tag `swift-mac-app`.
