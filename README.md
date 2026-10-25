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

The editor shows a line number gutter on the left, on by default: the number is right-aligned in at least 3 columns (more once the file has 1000+ lines), then one space, drawn dim (the number of the cursor's row is bold instead, so it is easy to find; only the number, no background, so it suits any theme), with a blank gutter next to the `~` rows past the end. F2 shows or hides it for all buffers; the setting is not saved between runs and is not per file. F2 does nothing in the changes view, file tree, buffer list and help, and is swallowed by an open Find / Go to line prompt. On a narrow terminal (fewer than 10 text columns would remain) the gutter is hidden so the text keeps its room. The gutter is only drawn; it is not part of the text, the cursor column in the status bar or search. Not done yet: highlighting the current line's number, relative numbers, a per-file or saved setting.

## Keys

| Key | Action |
|---|---|
| Home / End | Start / end of the line |
| PageUp / PageDown | Move the cursor one screenful up / down |
| Arrow keys | Move the cursor. Left and Right wrap across line ends; Up and Down remember the column across short lines |
| Printable keys | Insert text |
| Enter | New line, indented like the line you leave (its leading spaces and tabs, only as far as the cursor is; a selection is replaced; one undo step) |
| Alt-Left / Alt-Right | Move by word (Ctrl-Left / Ctrl-Right too; Alt-B / Alt-F is what macOS Terminal sends for Option+arrows; with Shift the selection grows). A word is letters, digits and `_`; punctuation and line breaks are skipped |
| Alt-Backspace / Alt-D | Delete the previous / next word (Ctrl-Backspace / Ctrl-Delete too; with a selection, just the selection); one undo step |
| Ctrl-] | Jump to the bracket matching the one under (else just before) the cursor: `()`, `[]`, `{}`, nesting counted per kind, strings and comments not skipped. Shift extends the selection to it. "No matching bracket" when there is none |
| Enter / `}` `)` `]` | Enter after `{`, `(` or `[` indents one level deeper (a tab if the row is tab-indented, else 4 spaces); between a typed pair like `{\|}` it puts the closer on its own line with the cursor in between. Typing a closer on a row that holds only indentation removes one level first. Any language, no config; one undo step each |
| Tab / Shift-Tab | With a selection over two or more rows, Tab indents every non-blank row by 4 spaces; Shift-Tab (selection or not) removes up to 4 leading spaces or one tab from the rows. The selection stays; one undo step |
| Ctrl-D | Duplicate the line below itself (the cursor moves onto the copy) |
| Alt-K | Delete the line, break included |
| Alt-Up / Alt-Down | Move the line up / down (the cursor follows) |
| Alt-/ or Ctrl-/ | Comment / uncomment the line with the file type's prefix (`//` for Rust, C, JS, TS, Go, Swift, Java, Kotlin...; `#` for Python, shell, Ruby, TOML, YAML...; `--` for SQL, Lua, Haskell). Comments at the shallowest indent, skips blank lines, and uncomments only when every non-blank line is commented. Ctrl-/ depends on the terminal (it may arrive as Ctrl-7 or Ctrl-_, both accepted) |
| Tab | Insert 4 spaces (an existing tab character is drawn up to the next multiple of 4) |
| Backspace / Delete | Delete before / under the cursor; at a line edge they join lines (CRLF breaks are removed whole) |
| Ctrl-G | Open the git changes view (again: back to the editor) |
| Ctrl-T | Open the file tree (again: back to the editor) |
| Ctrl-E | Open a file by name: the file finder, see [Open file by name](#open-file-by-name) |
| F4 | Find text in all files, see [Find in files](#find-in-files) |
| Ctrl-O | Open the buffer list (works from the git view and file tree too) |
| Ctrl-N / Ctrl-P | Next / previous buffer, wrapping around (also leaves the git view, tree or list) |
| Ctrl-W | Close the current buffer. A modified one warns first; press Ctrl-W again to close it without saving, any other key cancels. Closing the last buffer leaves one empty unnamed buffer. Ignored in the git view, tree and list |
| F2 | Show / hide line numbers (editor only; see [Line numbers](#line-numbers)) |
| F1 | Open the help screen (also `?` in the git view, file tree and buffer list) |
| Ctrl-S | Save the current buffer only |
| Ctrl-Z | Undo the last edit step (a run of typed characters, a run of Backspaces or Deletes, or one Enter is one step); says `Nothing to undo` when there is none |
| Ctrl-Y | Redo the last undone step (`Nothing to redo`); a new edit clears the redo steps |
| Shift + arrows / Home / End / PageUp / PageDown | Select text (see [Selection and clipboard](#selection-and-clipboard)) |
| Ctrl-A | Select everything |
| Ctrl-C / Ctrl-K / Ctrl-V | Copy / cut / paste (Ctrl-X is the command prompt, so cut is Ctrl-K) |
| Esc | Clear the selection |
| Ctrl-F | Find text (opens a prompt in the message row; see [Search](#search)) |
| F3 | Jump to the next match of the last search |
| Ctrl-R | Replace every occurrence of a term (in the selection, if there is one); see [Replace](#replace) |
| Ctrl-L | Go to a line number (prompt) |
| Ctrl-X | Command prompt (see [Commands](#commands)) |
| Ctrl-Q | Quit. If any buffer has unsaved changes it warns ("Unsaved changes in N buffers!"); press Ctrl-Q again to quit without saving. Any other key cancels the warning |

## Selection and clipboard

Hold Shift with an arrow key, Home, End, PageUp or PageDown to select: the anchor stays where you started and the cursor moves like the unshifted key. Ctrl-A selects everything. Selected cells are drawn in reverse video (over syntax colors; the gutter and a line break are never drawn selected). Any unshifted movement key, a search jump, Go to line, undo, redo, save, a buffer switch or Esc clears the selection. Typing, Tab or Enter replaces it; Backspace or Delete deletes it; each is one undo step together with the deletion. Esc in a Find / Go to line / Command prompt cancels the prompt and keeps the selection; accepting a Find or Go to line that moves the cursor clears it (one that finds nothing, or an empty one, keeps it like Esc).

- Ctrl-C copies, Ctrl-K cuts, Ctrl-V pastes (editor only; they do nothing in prompts, views and help). With nothing selected copy and cut show `Nothing selected`. Messages: `Copied N chars`, `Cut N chars`, with ` (ted only)` added when the system clipboard did not get the text. Paste replaces a selection. Copy is not an edit (nothing to undo); cut and paste are each one undo step.
- The system clipboard is used through `pbcopy`/`pbpaste` (macOS), `wl-copy`/`wl-paste` or `xclip -selection clipboard`, whichever is installed. Each call is limited to 2 seconds in total, and the tools run with `LC_ALL=en_US.UTF-8` (without a UTF-8 locale, `pbcopy` / `pbpaste` use MacRoman and corrupt non-ASCII text, for example in tmux without `LANG` or some ssh sessions). Text over 64 MiB is refused (`Clipboard too large`).
- Paste uses the system clipboard's text. If the clipboard works but holds no text (empty, an image, files, bytes that are not UTF-8) ted pastes nothing and says `Clipboard has no text`; if reading failed or timed out, `Clipboard read failed`.
- Fallback: ted also keeps an in-memory register that every copy and cut fills. It is used only when no clipboard tool is installed at all, or when the last copy never reached the system clipboard (`(ted only)`) and the system has nothing newer to say (empty or a failed read; text found there wins). So copy and paste inside ted work with no clipboard tool, and an old register never overrides what you copied elsewhere. With nothing at all, paste shows `Nothing to paste`. Text pasted from the clipboard has CRLF and lone CR turned into `\n` (so a file with CRLF line ends pasted into ted becomes LF text; bracketed paste does the same).
- A Find / Go to / Command prompt wider than the message row shows its end, led by `…`, so the typing position stays visible.
- Bracketed paste: ted asks the terminal to wrap pasted text in markers (and turns that off on exit), so a paste from the terminal (Cmd-V and so on) arrives as one piece: inserted whole as one undo step, in 5 MB in a fraction of a second, with CRLF and lone CR normalised and tabs kept. In an open Find or Command prompt it is added to the prompt with line breaks removed (so a pasted newline can never run a command; at most 4096 characters); in Go to line only its digits are kept; the git view, tree, buffer list and help ignore it. A terminal without bracketed paste types the text as keys, as before.
- Pasted control characters are stored as pasted and saved byte for byte, but drawn as `?` like any other.

Not done yet: mouse selection, rectangular selection, OSC 52 (copy over ssh), selection in the git view / tree / help, clipboard history.

## Commands

Ctrl-X opens a `Command:` prompt in the message row (editor only; it does nothing in the git view, file tree, buffer list and help). Type a command and press Enter; Esc cancels; Tab completes the command name (to the longest common prefix, plus a space when the name takes an argument). While you type the name, the matching names are listed after the prompt. Names are case-insensitive, arguments are not. An unambiguous prefix runs (`buf` = `buffers`); an ambiguous one shows `Ambiguous: save, saveas`; an unknown one shows `Unknown command: foo`. While the prompt is open it gets every key like the Find prompt (Ctrl-Q cancels it, then follows the quit rules).

| Command | Does |
|---|---|
| `save` (`w`) | Save, like Ctrl-S |
| `undo` / `redo` | Like Ctrl-Z / Ctrl-Y (`u` = `undo`, `red` = `redo`; `re` and `r` are ambiguous with `replace`) |
| `duplicate`, `deleteline`, `moveup`, `movedown` | The line operations above, for terminals where Option does not send Alt (`d` and `m` are ambiguous; use `du`, `del`, `moveu`, `moved`). With a selection they act on every row it touches (a selection ending at column 0 leaves that row out); each is one undo step |
| `open` | Open a file by name, like Ctrl-E |
| `grep` | Find text in all files, like F4 |
| `replace` | Open the Replace prompt, like Ctrl-R |
| `qreplace` | Like `replace`, but ask for each match (`qr` is enough); see [Replace](#replace) |
| `copy` / `cut` / `paste` / `selectall` | Like Ctrl-C / Ctrl-K / Ctrl-V / Ctrl-A (the selection survives opening the prompt, so `copy` works on it) |
| `saveas <path>` | Save an unnamed (or any) buffer under a new name; the buffer takes that name |
| `goto <n>` | Go to line n, like Ctrl-L |
| `lines [on\|off]` | Show / hide line numbers, like F2 (no argument toggles) |
| `next` / `prev` | Next / previous buffer |
| `close` | Close the buffer, with the Ctrl-W warning |
| `buffers` / `tree` / `git` / `help` | Open that view |
| `quit` (`q`) | Quit, with the Ctrl-Q warning |
| `wq` | Save, then quit only if the buffer was saved (an unnamed buffer does not quit); another modified buffer gives the usual unsaved-changes warning |

`saveas` resolves a relative path against ted's location (the root of the file tree). It never overwrites: an existing file or directory shows `Exists: <name>`, a file already open in another buffer shows `Already open: <name>`, an empty path shows `Usage: saveas <path>`, and a failed write (for example a missing directory) shows `Save failed: ...` and leaves the buffer's name and unsaved state as they were. On success the extension picks the highlighting. Not done yet: command history, completing arguments or paths, `:set`-style options, running commands from the other views, repeating the last command.

## Search

Ctrl-F opens a `Find:` prompt in the message row. If you searched before, the prompt is prefilled with that term (typed text is appended to it; Backspace clears it). There is one remembered term for all open buffers: accept `foo` (Enter) in one buffer and Ctrl-F and F3 in any other buffer use it; Esc and an empty Enter leave it as it was, and closing a buffer or reloading an unmodified one does not forget it. The cursor jumps to the first match at or after where the prompt opened, as you type. The search is plain text on a single line, case-insensitive unless the query contains an uppercase letter (smart case), and wraps around the end of the buffer (the prompt then shows `(wrapped)`); with no match it shows `(not found)`; while typing the cursor stays on the last match found, and Enter or Esc puts it back where you started. Ctrl-L opens `Go to line:` (digits only); the cursor moves to that line (counted from 1) only when you press Enter.

| Key (prompt) | Action |
|---|---|
| Enter | Accept: stay at the match (Find), or jump to the line (Go to line); the Find term is remembered |
| Esc | Cancel: the cursor and scroll go back to where they were |
| Down / Ctrl-F | Next match |
| Up | Previous match |
| F3 (in the editor) | Next match of the last search, without a prompt |
| Ctrl-Q | Cancel the prompt, then quit as usual |

While a prompt is open it gets every key: Ctrl-G, Ctrl-T, Ctrl-O, Ctrl-N, Ctrl-P, Ctrl-W, F1 and `?` do not open views or help (a `?` is part of the query). Ctrl-F, Ctrl-L and F3 do nothing in the git view, file tree, buffer list and help. Not done yet: highlighting matches after the prompt closes, regular expressions, queries that span lines, searching in the other views. Search history (only the last term is remembered) is not done yet. A full not-found scan of a 100,000-line file (about 55 characters per line, release build, measured by script) costs about 22 ms (case-insensitive query) or 14 ms (query with an uppercase letter), down from about 75 ms; extra characters typed after a not-found do not scan again.

### Match highlighting

While the Find prompt is open, every match in the visible rows is drawn underlined (typing, Backspace, Down / Up / Ctrl-F stepping; the cursor sits on the current match). It goes away when the prompt closes (Enter or Esc). Only the visible rows are scanned, so a 100,000-line file stays fast. A cell can carry one style, so on matched cells the underline replaces the syntax colour (the text keeps its colour elsewhere); a selected cell is drawn selected (reverse video) rather than underlined; the line number gutter is never marked.

## Replace

Ctrl-R (or the `replace` command) opens `Replace: ` in the message row, prefilled with the shared search term like Ctrl-F (typed text is appended; Backspace edits; nothing is searched or moved yet). Enter with a non-empty term moves to `With: ` (shown as `With (term): ` when the row has room); the new text may be empty, which deletes the matches. Enter again replaces every occurrence and shows `Replaced N occurrences` (`1 occurrence`), or `Not found: term` when there is none (nothing changes, cursor and selection go back). Esc at either step cancels and puts cursor, scroll and selection back; an empty term with Enter just closes the prompt; Ctrl-Q cancels like the other prompts. Ctrl-R does nothing in the changes view, file tree, buffer list and help, nor while a prompt is open.

- Matching is the same as Find: single line, smart case (case-insensitive unless the term has an uppercase letter), left to right without overlap (`aa` in `aaaa` is two matches). The new text is inserted exactly as typed (no case adaptation) and is not searched again (`a` to `aa` is safe).
- Scope: if text is selected when you press Ctrl-R, only matches fully inside the selection are replaced (a match cut by the selection edge is left alone); otherwise the whole buffer. The selection is cleared afterwards and the cursor goes to the start of the first replacement.
- The whole replacement is one undo step (Ctrl-Z restores the text and clears `[+]` if it was clean). The replace term becomes the shared search term, so Ctrl-F / F3 / the next Ctrl-R start from it.
- Memory: the single undo step stores the span from the first to the last replaced match twice, so a replace that touches the start and the end of a huge file uses that much history (the history is capped at 64 MiB and evicts older steps first).
- `qreplace` (Ctrl-X) asks first: after `With:` the first match is selected and the message row reads `Replace? (y)es (n)o (a)ll (q)uit [1/N]`. `y` replaces it, `n` skips it, `a` replaces it and all the rest, `q` or Esc stops; other keys are ignored. The buffer is changed only at the end, so the whole session is one undo step (and the undo step stores just the span of the chosen matches). The message then reads `Replaced K of N occurrences`; with nothing chosen nothing changes and cursor, scroll and selection go back. Scope and the shared search term work as for Ctrl-R. Ctrl-Q cancels the questions (nothing is applied) and follows the quit rules.
- Not done yet: regular expressions, case-preserving replace, multi-line terms.

## Open file by name

Ctrl-E (or the `open` command) lists the project's files under the directory ted was started in (or the directory of the file you gave it). In a git repository the list is what git knows: tracked plus untracked files, minus the ignored ones, so `target/` and `node_modules/` stay out. Outside a repository ted walks the directory instead, skipping hidden names, `target` and `node_modules` and not following directory symlinks. Files you used in this session (any buffer you typed in or switched to) come first, most recent first, so an empty query and equal matches show them at the top. The list is read again on every Ctrl-E and is cut at 100,000 files (the status row says `(list cut)`).

The message row is the query: `Open: edi_`. Type to filter; every character of the query has to appear in the path in that order, ignoring case (`edrs` finds `src/editor.rs`; `src/ap` finds `src/app.rs`). Better matches come first: a match inside the file name beats one in the folders, a file name starting with the query beats one that merely contains it, consecutive characters and word starts count, and a shorter path wins a tie. Backspace edits the query; a paste is added to it without line breaks. `q` and `?` are text here (F1 still opens help).

| Key (finder) | Action |
|---|---|
| Up / Down | Move the selection (typing puts it back on the best match) |
| PageUp / PageDown | Move 10 rows |
| Home / End | First / last match |
| Enter | Open the selected file in a buffer (switches to it if already open) and return to the editor; a file that vanished shows `File no longer exists` and the finder stays |
| Esc / Ctrl-E | Back to the editor, which is as it was |

Ctrl-N, Ctrl-P, Ctrl-O and F1 work as elsewhere; Ctrl-Q follows the quit rules. Ctrl-E does nothing while a prompt is open.

## Find in files

F4 (or the `grep` command) searches the text of the project's files: the same list as the file finder (tracked plus untracked files in a git repository, else a directory walk), read again on every search. The message row is the query and starts with the last term you searched for in any buffer. Type, then Enter: the matching lines are listed as `path:line: text` (indent trimmed), in file order, and the first is selected. Enter again opens the selected hit's file in a buffer (or switches to it) with the cursor on that line, and the term becomes the shared search term, so F3 and Ctrl-F continue from it.

Matching is plain text with smart case like Find: an all-lowercase query ignores case, any capital makes it exact. A line is listed once however often it matches. Files that are not UTF-8, contain a NUL byte or are over 2 MiB are skipped. The list stops at 2,000 hits (the status says `first 2000`).

| Key (find in files) | Action |
|---|---|
| Enter | Search when the query is new or was edited, otherwise open the selected hit |
| Up / Down, PageUp / PageDown, Home / End | Move the selection |
| Backspace, typing, paste | Edit the query; the old hits stay until Enter, marked `Enter searches again` |
| Esc / F4 | Back to the editor, which is as it was |

`q` and `?` are text here (F1 still opens help); Ctrl-N, Ctrl-P, Ctrl-O work as elsewhere; Ctrl-Q follows the quit rules. F4 does nothing while a prompt is open.

## Help

Press F1 to see every key. The editor shows `F1: help` in its message row until your first key press. F1 works from the editor, the git view, the file tree and the buffer list, and `?` also opens it from those three views (in the editor `?` is just text). The help screen lists the keys in sections (Editor, Quit, Selection and clipboard, Find / go to line prompt, Replace prompt, Help, Files and buffers, Views); lines longer than the window are cut, not wrapped. Other keys (Ctrl-S, typing and so on) are ignored while help is showing. Opened from the commit picker (`b` in the git view), closing help returns to the picker.

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
src/command.rs   Command prompt logic: names, prefix matching, Tab completion, parsing.
src/clipboard.rs Clipboard trait: system (pbcopy/wl-copy/xclip behind a runner seam, 2 s limit) and in-memory.
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

Design rule: all logic lives in the library (`buffer`, `bufferlist`, `editor`, `git`, `gitview`, `help`, `filetree`, `text`, `app`, `screen`, `highlight`, `command`) so it can be tested without a terminal. `main.rs` only reads keys and prints.

Known simplifications:
- Undo history is per buffer and lives as long as the buffer; it is lost when an unmodified buffer is reloaded because the file changed on disk (reopening a saved buffer whose file text is unchanged keeps history and cursor). Saving and `saveas` keep it. Ctrl-Z / Ctrl-Y are ignored in the git view, file tree, buffer list, help and while a prompt is open. Cursor moves are not undone.
- Columns count Rust `char`s, not display cells, so emoji and CJK characters misalign the cursor.
- "Modified" means the text differs from the last saved state in the undo history: typing and then undoing it is unmodified again; undoing past a save is modified.

## Testing

```
cargo test                                   # 746 unit tests
cargo clippy --all-targets                   # clean
cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted
```

The pty smoke test runs the real binary: type, save, quit, the unsaved-changes warning, editing an existing file, refusing a non-UTF-8 file, the usage error, and that the screen is restored on exit. It also opens the git view in a temporary repo (changed and untracked files, a `+` diff line, `q` back to the editor) and in a non-repo directory, and checks that a slow `git` never blocks keys. A final scenario opens the file tree in a temporary directory: Ctrl-T lists entries and a dotfile, Right expands a directory, Enter opens a file, Ctrl-T twice returns to the editor, Ctrl-Q exits 0. A last scenario opens a `.rs` file and checks that the keyword, string and comment are preceded by the magenta, green and dim sequences, and that a `.txt` file with identical text has no color or dim sequences. The last one opens a `.txt` file containing `ESC[31mRED ESC[2J END` and checks that neither sequence reaches the terminal (apart from ted's own clear on entering the alternate screen) and that `?[31mRED` and `END` are drawn. A final scenario opens `a.txt` and `b.txt` through the tree, checks `[2/2]`, edits the first, checks the Ctrl-O list (names, `[+]`, `>` marker), Enter back, the Ctrl-W warning, the Ctrl-Q warning with `1 buffer`, and a second Ctrl-Q exiting 0. A scenario for PageDown/PageUp checks the cursor jumps a screenful. The last scenario checks the `F1: help` hint, F1 showing the Help screen with `Ctrl-S` and `Ctrl-Q`, `q` returning to the editor text and exit 0. A search scenario opens a 200-line file and checks Ctrl-F "line 150" + Enter (status `150:1`), Ctrl-L "10" + Enter (status `10:1`), a second Ctrl-F prefilled with `line 150`, Backspace clearing it, "zzz" showing `not found`, Esc keeping `10:1`, "line 15" + Enter (`15:1`), F3 jumping to `150:1`, and exit 0. A line numbers scenario opens a 12-line file and checks `  1 line 0` and ` 12 line 11` on start (gutter dim), that F2 hides the gutter while `line 0` stays, that F2 again restores it, and that F2 leaves the buffer unmodified (exit 0); style escapes are stripped before looking because the gutter is its own dim span. The `.txt` highlighting check now allows the dim gutter and nothing else dim. A command prompt scenario opens a 12-line file and checks that Ctrl-X shows `Command: `, that `lines` hides and restores the gutter, that `zzz` shows `Unknown command: zzz` and exit 0; a saveas scenario starts without a file in a temporary directory, types text, runs `saveas out.txt`, and checks the status shows `out.txt`, `Saved 1 lines`, the file holds the typed text, and Ctrl-Q exits 0. That is 104 checks in total (the 94 before, plus 6 for the command prompt and 4 for saveas). An undo scenario opens a one-line file `abc`, types `XY` (`XYabc`, `[+]`), checks Ctrl-Z gives `abc` without `[+]`, Ctrl-Y brings `XYabc` and `[+]` back, a further Ctrl-Z shows `Nothing to undo`, and Ctrl-Q exits 0 (6 more checks, 110 in total). A shared search scenario opens `a.txt` and `b.txt` through the tree, accepts `needle` in the first (status `2:1`), switches with Ctrl-N, checks Ctrl-F in the second shows `Find: needle`, Esc keeps `1:1`, F3 jumps to `3:1`, and exit 0 (7 more checks, 117 in total). A selection and clipboard scenario runs ted with a `PATH` that has no clipboard tool (so the in-memory register is used): bracketed paste enabled on entry, Shift+Right x3 draws a reversed span over `fir`, typing `X` replaces it, Ctrl-Z restores, Ctrl-A Ctrl-K cuts everything (`[+]`, `Cut 34 chars`), Ctrl-V pastes it back, a bracketed multi-line paste lands as three lines and one Ctrl-Z removes it, a bracketed `wq` plus newline in the Ctrl-X prompt fills the prompt without running it and saves nothing, and exit 0 (13 more checks, 130 in total). A replace scenario opens a 12-line file with six `foo` lines and checks that Ctrl-F `foo` underlines the matches (`\x1b[4m` before `foo`) and that none are drawn after Enter, that Ctrl-R is prefilled with `foo`, that Enter asks `With (foo): `, that `bar` + Enter shows `Replaced 6 occurrences` with `bar` in place of every `foo` and `[+]`, that one Ctrl-Z restores the `foo` lines, that Esc during `With:` changes nothing, and exit 0 (12 more checks, 142 in total). A `qreplace` part of the same scenario runs `qreplace` `foo` -> `bar` and checks the `[1/6]` question, the reverse-video match, `[2/6]` after `y`, `Replaced 5 of 6 occurrences` after `n` then `a`, the mixed text, and one Ctrl-Z restoring everything (6 more, 148 in total).

The buffer commands not in that scenario (Ctrl-N / Ctrl-P wrapping, closing, opening an already-open file, Ctrl-O from the git view or tree, list scrolling, tiny windows) are covered by unit tests only. The file tree's Ctrl-G / Ctrl-T cross-switching, its once-a-second refresh, the `(cannot read)` row and rooting for `ted <file>` are covered by unit tests only, not by the pty run.

Verified by hand in a real terminal: save, quit, add text, delete text. Unit tests and the pty test do not prove look and feel, window resizing, Option/Alt keys, the git view's live refresh while another process edits files, its colors, or its behavior in a very large repo; how the dim line number gutter looks on a real terminal and in your theme, the highlighting colors on a real terminal, and their readability in your theme (including dim comments) are also unchecked; likewise the help screen's look and scrolling feel on a real terminal (F1 is only checked through the pty, not on every terminal or over ssh/tmux) and the buffer list's look, scrolling and resize, switching buffers by hand, and the file tree's colors, resize, live refresh while another process changes files, Ctrl-G / Ctrl-T cross-switching, a very large directory and symlinked directories; those are still unchecked.

## Not yet

mouse selection, OSC 52, rectangular selection, clipboard history, search extras (persistent match highlight, regex, case-preserving replace, multi-line queries, search in other views, search history), command history and argument completion in the command prompt, correct cursor alignment for wide characters, syntax highlighting for languages other than Rust. In the git view: wide-character alignment, mouse, word-level diffs, a configurable refresh rate. Buffers: closing or saving from the buffer list, jumping by number, reopening a closed buffer, reloading buffers on disk change when they are not reopened. In the file tree: a sidebar next to the editor, status marks, create/rename/delete, filtering/search, a hidden-file toggle.

## History

- An earlier attempt was a SwiftUI Mac app (default stack for other projects). It was replaced by this Rust terminal editor and is kept at the git tag `swift-mac-app`.
- Shell setup: Homebrew's `rustup` is keg-only, so `~/src/config/zsh/zshenv` (symlinked as `~/.zshenv`) puts `~/.cargo/bin` and `/opt/homebrew/opt/rustup/bin` on `PATH`.
