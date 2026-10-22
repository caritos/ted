# ted

A small text editor for the terminal, written in Rust. It is built for one person's daily use, with one goal: it just works, with no configuration.

Status: first working version (2026-10-22). You can open, edit and save a file. It is deliberately minimal; see [Not yet](#not-yet).

## Install and run

Requires Rust (installed here through Homebrew's `rustup`).

```
cargo run --release -- somefile.txt     # run from the repo
cargo install --path .                  # installs `ted` to ~/.cargo/bin
ted somefile.txt
```

`ted` with no argument opens an unnamed buffer. `ted a b` is a usage error (exit 2). A file that does not exist opens as an empty buffer and is created on first save. A file that exists but is not valid UTF-8 is refused before the terminal is touched (exit 1), so it can never be corrupted.

`ted <directory>` opens an empty, unnamed buffer located in that directory (so the git view below looks at it). `ted <file>` uses the file's parent directory.

## Git changes view

Press Ctrl-G in the editor to open a read-only git view: changed files on the left (status letter `M`/`A`/`D`/`?`, path, age, newest first) and the diff of the selected file on the right. It refreshes about once a second, so you can watch another process (for example Claude) change files. `q`, Esc or Ctrl-G again returns to the editor exactly as it was. Outside a git repository it shows "not a git repository".

"Changed" means different from the baseline commit, which is `HEAD` at the moment you first open the git view. Commits made afterwards do not hide changes. Untracked files respect `.gitignore`.

| Key (git view) | Action |
|---|---|
| Up / Down | Select a file |
| PageUp / PageDown | Scroll the diff |
| Enter | Open the selected file in the editor (refused with a message if the current buffer has unsaved changes) |
| `r` | Reset the baseline to the current `HEAD` |
| `b` | Pick a recent commit as the baseline (Enter picks, Esc closes) |
| `q` / Esc / Ctrl-G | Back to the editor (closes the commit picker first if open) |
| Ctrl-T | Jump to the file tree (closes the commit picker) |

The git view only reads: it never stages, commits or changes the repo or working tree.

## File tree

Press Ctrl-T in the editor to open a read-only file tree rooted at the same directory the git view looks at: the current directory for `ted` alone, `<dir>` for `ted <dir>`, and the file's parent for `ted <file>`. The status row shows that directory's name. Directories come first, then files, each sorted case-insensitively. Dotfiles are shown; `.git` is hidden. It refreshes about once a second while showing. A directory that cannot be read shows a dim `(cannot read)` row and a message. The tree only reads: it never creates, renames or deletes anything.

| Key (file tree) | Action |
|---|---|
| Up / Down | Move the selection |
| PageUp / PageDown | Move 10 rows |
| Home / End | First / last row |
| Right | Expand a directory (on an expanded one, move to its first child) |
| Left | Collapse a directory (on a file or collapsed one, select the parent) |
| Enter | Toggle a directory; open a file in the editor (refused with a message for unsaved changes, a FIFO, socket or device, or a file over 64 MiB) |
| `q` / Esc / Ctrl-T | Back to the editor |
| Ctrl-G | Jump to the changes view (and Ctrl-T in that view jumps to the tree) |

## Syntax highlighting

Files ending in `.rs` are drawn with colors from the terminal's ANSI palette, so they follow your terminal theme: keywords magenta, types yellow, strings green, comments dim (your normal text color, faded), numbers cyan, and macros, attributes and lifetimes blue. It is always on and has no configuration. Every other file, and unnamed buffers, is drawn plain for now. The git diff and the file tree are not highlighted. Colors only change how text is drawn, never the text or the cursor. The highlight state of each line is cached and only recomputed from the first edited line, so editing or scrolling near the end of a 100,000-line Rust file stays well under a millisecond per frame (measured by script); the first frame after jumping to the bottom of such a file takes about 90 ms once.

## Keys

| Key | Action |
|---|---|
| Arrow keys | Move the cursor. Left and Right wrap across line ends; Up and Down remember the column across short lines |
| Printable keys | Insert text |
| Enter | New line |
| Tab | Insert 4 spaces |
| Backspace / Delete | Delete before / under the cursor; at a line edge they join lines (CRLF breaks are removed whole) |
| Ctrl-G | Open the git changes view (again: back to the editor) |
| Ctrl-T | Open the file tree (again: back to the editor) |
| Ctrl-S | Save |
| Ctrl-Q | Quit. With unsaved changes it warns first; press Ctrl-Q again to quit without saving. Any other key cancels the warning |

The bottom of the screen shows a status bar (file name, `[+]` when modified, `line:column`) and a message row (for example "Saved 3 lines", or "Save failed: ...").

Saving writes to a temporary sibling file and renames it over the target, so a failed write never leaves a half-written file. Symlinks are followed and existing file permissions are kept.

Control characters in a file, its name or a message are never sent to the terminal: each one is drawn as a single `?` (a tab as a single space), so a file containing escape sequences cannot recolor, clear or retitle your terminal. Only the drawing changes; the text itself and the bytes saved stay exactly as they were. Tabs are not expanded to tab stops yet.

Very small windows (fewer than 3 rows, or 0 columns) show only what fits, and never crash, even after scrolling.

## How it is built

```
src/buffer.rs    Rope-backed text, cursor, load and atomic save. No terminal code.
src/editor.rs    Key handling, scrolling, status and messages, quit confirmation.
                 Editor::view(width, height) returns a plain View value to draw.
src/git.rs       Read-only `git` CLI wrapper behind a trait (status, diff, log).
src/gitview.rs   Git view state: file list, selection, diff scroll, commit picker.
src/text.rs      Shared text helpers (width fitting, truncation).
src/filetree.rs  Read-only file tree state: expansion, selection, rendering.
src/app.rs       Owns the editor, git view and file tree; switches between them.
src/screen.rs    Plain Screen value both modes render to.
src/highlight.rs Rust tokenizer: classifies each char of a line (std only).
src/main.rs      Thin crossterm layer: raw mode, read keys, draw the View,
                 restore the terminal on exit, error and panic.
scripts/pty_smoke.py   Drives the release binary through a pseudo-terminal.
```

Dependencies: `crossterm` (terminal input and output) and `ropey` (text storage). There is no `ratatui` on purpose; the editor draws its own screen.

Design rule: all logic lives in the library (`buffer`, `editor`, `git`, `gitview`, `filetree`, `text`, `app`, `screen`, `highlight`) so it can be tested without a terminal. `main.rs` only reads keys and prints.

Known simplifications:
- Columns count Rust `char`s, not display cells, so emoji and CJK characters misalign the cursor.
- "Modified" means any edit since the last save. There is no undo, so typing and then deleting still counts.

## Testing

```
cargo test                                   # 246 unit tests
cargo clippy --all-targets                   # clean
cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted
```

The pty smoke test runs the real binary: type, save, quit, the unsaved-changes warning, editing an existing file, refusing a non-UTF-8 file, the usage error, and that the screen is restored on exit. It also opens the git view in a temporary repo (changed and untracked files, a `+` diff line, `q` back to the editor) and in a non-repo directory, and checks that a slow `git` never blocks keys. A final scenario opens the file tree in a temporary directory: Ctrl-T lists entries and a dotfile, Right expands a directory, Enter opens a file, Ctrl-T twice returns to the editor, Ctrl-Q exits 0. A last scenario opens a `.rs` file and checks that the keyword, string and comment are preceded by the magenta, green and dim sequences, and that a `.txt` file with identical text has no color or dim sequences. The last one opens a `.txt` file containing `ESC[31mRED ESC[2J END` and checks that neither sequence reaches the terminal (apart from ted's own clear on entering the alternate screen) and that `?[31mRED` and `END` are drawn. That is 52 checks in total (27 before the tree, 12 for the tree, 8 for highlighting, 5 for escape sequences).

The file tree's Ctrl-G / Ctrl-T cross-switching, its once-a-second refresh, the `(cannot read)` row and rooting for `ted <file>` are covered by unit tests only, not by the pty run.

Verified by hand in a real terminal: save, quit, add text, delete text. Unit tests and the pty test do not prove look and feel, window resizing, Option/Alt keys, the git view's live refresh while another process edits files, its colors, or its behavior in a very large repo; the highlighting colors on a real terminal, and their readability in your theme (including dim comments) are also unchecked; likewise the file tree's colors, resize, live refresh while another process changes files, Ctrl-G / Ctrl-T cross-switching, a very large directory and symlinked directories; those are still unchecked.

## Not yet

Home/End, PageUp/PageDown, undo, search, tab expansion to tab stops (a tab currently shows as one space), a save-as prompt for unnamed buffers, correct cursor alignment for wide characters, syntax highlighting for languages other than Rust. In the git view: wide-character alignment, mouse, word-level diffs, a configurable refresh rate. In the file tree: a sidebar next to the editor, status marks, create/rename/delete, filtering/search, a hidden-file toggle.

## History

- An earlier attempt was a SwiftUI Mac app (default stack for other projects). It was replaced by this Rust terminal editor and is kept at the git tag `swift-mac-app`.
- Shell setup: Homebrew's `rustup` is keg-only, so `~/src/config/zsh/zshenv` (symlinked as `~/.zshenv`) puts `~/.cargo/bin` and `/opt/homebrew/opt/rustup/bin` on `PATH`.
