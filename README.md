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
| Enter | Open the selected file in a buffer (switches to it if already open) |
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
| Enter | Toggle a directory; open a file in a buffer (refused with a message for a FIFO, socket, device or a file over 64 MiB) |
| `q` / Esc / Ctrl-T | Back to the editor |
| Ctrl-G | Jump to the changes view (and Ctrl-T in that view jumps to the tree) |

## Syntax highlighting

Files ending in `.rs` are drawn with colors from the terminal's ANSI palette, so they follow your terminal theme: keywords magenta, types yellow, strings green, comments dim (your normal text color, faded), numbers cyan, and macros, attributes and lifetimes blue. It is always on and has no configuration. Every other file, and unnamed buffers, is drawn plain for now. The git diff and the file tree are not highlighted. Colors only change how text is drawn, never the text or the cursor. The highlight state of each line is cached and only recomputed from the first edited line, so editing or scrolling near the end of a 100,000-line Rust file stays well under a millisecond per frame (measured by script); the first frame after jumping to the bottom of such a file takes about 90 ms once.

## Line numbers

The editor shows a line number gutter on the left, on by default: the number is right-aligned in at least 3 columns (more once the file has 1000+ lines), then one space, drawn dim, with a blank gutter next to the `~` rows past the end. F2 shows or hides it for all buffers; the setting is not saved between runs and is not per file. F2 does nothing in the changes view, file tree, buffer list and help, and is swallowed by an open Find / Go to line prompt. On a narrow terminal (fewer than 10 text columns would remain) the gutter is hidden so the text keeps its room. The gutter is only drawn; it is not part of the text, the cursor column in the status bar or search. Not done yet: highlighting the current line's number, relative numbers, a per-file or saved setting.

## Keys

| Key | Action |
|---|---|
| Home / End | Start / end of the line |
| PageUp / PageDown | Move the cursor one screenful up / down |
| Arrow keys | Move the cursor. Left and Right wrap across line ends; Up and Down remember the column across short lines |
| Printable keys | Insert text |
| Enter | New line |
| Tab | Insert 4 spaces (an existing tab character is drawn up to the next multiple of 4) |
| Backspace / Delete | Delete before / under the cursor; at a line edge they join lines (CRLF breaks are removed whole) |
| Ctrl-G | Open the git changes view (again: back to the editor) |
| Ctrl-T | Open the file tree (again: back to the editor) |
| Ctrl-O | Open the buffer list (works from the git view and file tree too) |
| Ctrl-N / Ctrl-P | Next / previous buffer, wrapping around (also leaves the git view, tree or list) |
| Ctrl-W | Close the current buffer. A modified one warns first; press Ctrl-W again to close it without saving, any other key cancels. Closing the last buffer leaves one empty unnamed buffer. Ignored in the git view, tree and list |
| F2 | Show / hide line numbers (editor only; see [Line numbers](#line-numbers)) |
| F1 | Open the help screen (also `?` in the git view, file tree and buffer list) |
| Ctrl-S | Save the current buffer only |
| Ctrl-F | Find text (opens a prompt in the message row; see [Search](#search)) |
| F3 | Jump to the next match of the last search |
| Ctrl-L | Go to a line number (prompt) |
| Ctrl-Q | Quit. If any buffer has unsaved changes it warns ("Unsaved changes in N buffers!"); press Ctrl-Q again to quit without saving. Any other key cancels the warning |

## Search

Ctrl-F opens a `Find:` prompt in the message row. If you searched before, the prompt is prefilled with that term (typed text is appended to it; Backspace clears it). The cursor jumps to the first match at or after where the prompt opened, as you type. The search is plain text on a single line, case-insensitive unless the query contains an uppercase letter (smart case), and wraps around the end of the buffer (the prompt then shows `(wrapped)`); with no match it shows `(not found)`; while typing the cursor stays on the last match found, and Enter or Esc puts it back where you started. Ctrl-L opens `Go to line:` (digits only); the cursor moves to that line (counted from 1) only when you press Enter.

| Key (prompt) | Action |
|---|---|
| Enter | Accept: stay at the match (Find), or jump to the line (Go to line); the Find term is remembered |
| Esc | Cancel: the cursor and scroll go back to where they were |
| Down / Ctrl-F | Next match |
| Up | Previous match |
| F3 (in the editor) | Next match of the last search, without a prompt |
| Ctrl-Q | Cancel the prompt, then quit as usual |

While a prompt is open it gets every key: Ctrl-G, Ctrl-T, Ctrl-O, Ctrl-N, Ctrl-P, Ctrl-W, F1 and `?` do not open views or help (a `?` is part of the query). Ctrl-F, Ctrl-L and F3 do nothing in the git view, file tree, buffer list and help. Not done yet: highlighting all matches, replace, regular expressions, queries that span lines, searching in the other views. The search term is per buffer and is forgotten when an unmodified buffer is reloaded from disk (one shared term is not done yet). A full not-found scan of a 100,000-line file (about 55 characters per line, release build, measured by script) costs about 22 ms (case-insensitive query) or 14 ms (query with an uppercase letter), down from about 75 ms; extra characters typed after a not-found do not scan again.

## Help

Press F1 to see every key. The editor shows `F1: help` in its message row until your first key press. F1 works from the editor, the git view, the file tree and the buffer list, and `?` also opens it from those three views (in the editor `?` is just text). The help screen lists the keys in sections (Editor, Quit, Find / go to line prompt, Help, Files and buffers, Views); lines longer than the window are cut, not wrapped. Other keys (Ctrl-S, typing and so on) are ignored while help is showing. Opened from the commit picker (`b` in the git view), closing help returns to the picker.

| Key (help) | Action |
|---|---|
| Up / Down | Scroll one line |
| PageUp / PageDown | Scroll one page |
| Home / End | Top / bottom |
| `q` / Esc / F1 / `?` | Back to where you opened it |
| Ctrl-Q | Quit, with the usual unsaved-changes warning (shown in the help's message row) |
| Ctrl-N / Ctrl-P / Ctrl-O / Ctrl-G / Ctrl-T | Leave help and do what they do elsewhere |

Ctrl-W is ignored in help. Each visit starts at the top.

## Buffers

`ted` holds several files at once. Opening a file from the git view or file tree adds a buffer and switches to it, so unsaved edits in the buffer you were in are never lost or refused. Opening a file that is already open (same file after resolving `./x` and symlinks) switches to it instead of adding a second copy; if that buffer has no unsaved edits it is re-read from disk first (so changes made elsewhere show up), and if it has edits they are kept as they are. An unnamed, empty, unmodified buffer is replaced by the first file opened. When more than one buffer is open the status bar shows `[2/3]` after the file name.

Ctrl-O opens the buffer list: one row per buffer (`  1  a.txt`, the current one marked with `>`, ` [+]` when modified), with the selection starting on the current buffer.

| Key (buffer list) | Action |
|---|---|
| Up / Down | Move the selection |
| Home / End | First / last buffer |
| Enter | Switch to the selected buffer and return to the editor |
| `q` / Esc / Ctrl-O | Back to the editor |
| Ctrl-G / Ctrl-T | Jump to the git view / file tree |

The bottom of the screen shows a status bar (file name, `[+]` when modified, `line:column`) and a message row (for example "Saved 3 lines", or "Save failed: ...").

Saving writes to a temporary sibling file and renames it over the target, so a failed write never leaves a half-written file. Symlinks are followed and existing file permissions are kept.

Control characters in a file, its name or a message are never sent to the terminal: each one is drawn as a single `?`, so a file containing escape sequences cannot recolor, clear or retitle your terminal. Only the drawing changes; the text itself and the bytes saved stay exactly as they were. In the editor a tab is drawn as spaces up to the next multiple of 4 columns (the cursor sits on the first cell of a tab); the git view, file tree and messages still draw a tab as one space. The status `row:col` counts characters, not display columns.

Very small windows (fewer than 3 rows, or 0 columns) show only what fits, and never crash, even after scrolling.

## How it is built

```
src/buffer.rs    Rope-backed text, cursor, load and atomic save. No terminal code.
src/editor.rs    Key handling, scrolling, status and messages.
                 Editor::view(width, height) returns a plain View value to draw.
src/git.rs       Read-only `git` CLI wrapper behind a trait (status, diff, log).
src/gitview.rs   Git view state: file list, selection, diff scroll, commit picker.
src/text.rs      Shared text helpers (width fitting, truncation, `sanitize`, tab-stop layout).
src/filetree.rs  Read-only file tree state: expansion, selection, rendering.
src/bufferlist.rs Buffer list state (selection, keys) and rendering.
src/help.rs      Help screen: the key table, scrolling and rendering.
src/app.rs       Owns the buffers (one Editor each), git view, file tree and buffer
                 list; switches between them; Ctrl-N/P/O/W/Q.
src/screen.rs    Plain Screen value both modes render to.
src/highlight.rs Rust tokenizer: classifies each char of a line (std only).
src/main.rs      Thin crossterm layer: raw mode, read keys, draw the View,
                 restore the terminal on exit, error and panic.
scripts/pty_smoke.py   Drives the release binary through a pseudo-terminal.
```

Dependencies: `crossterm` (terminal input and output) and `ropey` (text storage). There is no `ratatui` on purpose; the editor draws its own screen.

Design rule: all logic lives in the library (`buffer`, `bufferlist`, `editor`, `git`, `gitview`, `help`, `filetree`, `text`, `app`, `screen`, `highlight`) so it can be tested without a terminal. `main.rs` only reads keys and prints.

Known simplifications:
- Columns count Rust `char`s, not display cells, so emoji and CJK characters misalign the cursor.
- "Modified" means any edit since the last save. There is no undo, so typing and then deleting still counts.

## Testing

```
cargo test                                   # 393 unit tests
cargo clippy --all-targets                   # clean
cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted
```

The pty smoke test runs the real binary: type, save, quit, the unsaved-changes warning, editing an existing file, refusing a non-UTF-8 file, the usage error, and that the screen is restored on exit. It also opens the git view in a temporary repo (changed and untracked files, a `+` diff line, `q` back to the editor) and in a non-repo directory, and checks that a slow `git` never blocks keys. A final scenario opens the file tree in a temporary directory: Ctrl-T lists entries and a dotfile, Right expands a directory, Enter opens a file, Ctrl-T twice returns to the editor, Ctrl-Q exits 0. A last scenario opens a `.rs` file and checks that the keyword, string and comment are preceded by the magenta, green and dim sequences, and that a `.txt` file with identical text has no color or dim sequences. The last one opens a `.txt` file containing `ESC[31mRED ESC[2J END` and checks that neither sequence reaches the terminal (apart from ted's own clear on entering the alternate screen) and that `?[31mRED` and `END` are drawn. A final scenario opens `a.txt` and `b.txt` through the tree, checks `[2/2]`, edits the first, checks the Ctrl-O list (names, `[+]`, `>` marker), Enter back, the Ctrl-W warning, the Ctrl-Q warning with `1 buffer`, and a second Ctrl-Q exiting 0. A scenario for PageDown/PageUp checks the cursor jumps a screenful. The last scenario checks the `F1: help` hint, F1 showing the Help screen with `Ctrl-S` and `Ctrl-Q`, `q` returning to the editor text and exit 0. A search scenario opens a 200-line file and checks Ctrl-F "line 150" + Enter (status `150:1`), Ctrl-L "10" + Enter (status `10:1`), a second Ctrl-F prefilled with `line 150`, Backspace clearing it, "zzz" showing `not found`, Esc keeping `10:1`, "line 15" + Enter (`15:1`), F3 jumping to `150:1`, and exit 0. A line numbers scenario opens a 12-line file and checks `  1 line 0` and ` 12 line 11` on start (gutter dim), that F2 hides the gutter while `line 0` stays, that F2 again restores it, and that F2 leaves the buffer unmodified (exit 0); style escapes are stripped before looking because the gutter is its own dim span. The `.txt` highlighting check now allows the dim gutter and nothing else dim. That is 89 checks in total (27 before the tree, 12 for the tree, 9 for highlighting, 5 for escape sequences, 10 for buffers, 3 for paging, 5 for help, 12 for search, 6 for line numbers).

The buffer commands not in that scenario (Ctrl-N / Ctrl-P wrapping, closing, opening an already-open file, Ctrl-O from the git view or tree, list scrolling, tiny windows) are covered by unit tests only. The file tree's Ctrl-G / Ctrl-T cross-switching, its once-a-second refresh, the `(cannot read)` row and rooting for `ted <file>` are covered by unit tests only, not by the pty run.

Verified by hand in a real terminal: save, quit, add text, delete text. Unit tests and the pty test do not prove look and feel, window resizing, Option/Alt keys, the git view's live refresh while another process edits files, its colors, or its behavior in a very large repo; how the dim line number gutter looks on a real terminal and in your theme, the highlighting colors on a real terminal, and their readability in your theme (including dim comments) are also unchecked; likewise the help screen's look and scrolling feel on a real terminal (F1 is only checked through the pty, not on every terminal or over ssh/tmux) and the buffer list's look, scrolling and resize, switching buffers by hand, and the file tree's colors, resize, live refresh while another process changes files, Ctrl-G / Ctrl-T cross-switching, a very large directory and symlinked directories; those are still unchecked.

## Not yet

undo, search extras (match highlight, replace, regex, multi-line queries, search in other views, one search term shared across buffers), a save-as prompt for unnamed buffers, correct cursor alignment for wide characters, syntax highlighting for languages other than Rust. In the git view: wide-character alignment, mouse, word-level diffs, a configurable refresh rate. Buffers: closing or saving from the buffer list, jumping by number, reopening a closed buffer, reloading buffers on disk change when they are not reopened. In the file tree: a sidebar next to the editor, status marks, create/rename/delete, filtering/search, a hidden-file toggle.

## History

- An earlier attempt was a SwiftUI Mac app (default stack for other projects). It was replaced by this Rust terminal editor and is kept at the git tag `swift-mac-app`.
- Shell setup: Homebrew's `rustup` is keg-only, so `~/src/config/zsh/zshenv` (symlinked as `~/.zshenv`) puts `~/.cargo/bin` and `/opt/homebrew/opt/rustup/bin` on `PATH`.
