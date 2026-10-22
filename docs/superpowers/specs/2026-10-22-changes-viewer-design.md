# Git changes view (`ted <directory>`, then a key)

## Intent
I program with Claude and want to watch what it changes without leaving my editor. Running `ted <directory>` opens the normal editor with an empty buffer, with that directory as its location. A key opens a read-only git view: a live list of files that changed in the git repo, and the diff of the selected file. Today `ted <file>` opens the editor on a file; a directory argument is the new case, and the git view is a mode of the editor, not a separate program.

Success: I run it beside a Claude session and, at a glance, see which files are changing and what changed, even after Claude has committed them.

## Behavior
- **Opening.** `ted <dir>` starts the editor on an empty, unnamed buffer whose location (the directory the git view looks at) is `<dir>`. `ted <file>` uses the file's parent directory as the location, so the git view works there too. The git view key (Ctrl-G, my call) switches to the git view; a key (`q` or Esc) returns to the editor exactly as it was.
- **Baseline.** When the git view is first opened in a session, remember the current `HEAD` commit. "Changed" means any file that differs between that commit and the working tree. Commits Claude makes after the baseline do not hide changes.
- **Left pane: file list.** One row per changed file: status letter (`M` modified, `A` added, `D` deleted, `?` untracked), path, and age ("2s", "14s", "3m"). Sorted newest first. Age comes from the file's modified time (the builder's call; git has no such data). Deleted files have no mtime and sort by the time they were first noticed.
- **Right pane: diff.** The unified diff of the selected file against the baseline, read-only, scrollable. Untracked files show as all-added.
- **Live.** The list and diff refresh on their own about once a second. No keypress needed. The selection stays on the same file when the list reorders (key by path, not row position).
- **Keys.**
  - Up/Down: move selection in the list.
  - PageUp/PageDown (or similar): scroll the diff.
  - Enter: open the selected file in the editor, replacing the current buffer, and leave the git view. If the current buffer has unsaved changes, Enter does nothing except show a message (my call; never discard edits silently).
  - A key to reset the baseline to now: baseline becomes the current `HEAD`. (Uncommitted edits already in the working tree still show, since they differ from `HEAD`.)
  - A key to open a list of recent commits; choosing one makes it the baseline. Modeled on the commit list in IntelliJ's Git Log (user's reference screenshot): one row per commit, newest first, showing short hash, subject, date and time, and branch/tag labels next to the subject (e.g. `main`, `origin/main`, `swift-mac-app`). No author column (my call: nearly every commit here is by the same person); no graph lines.
  - `q` or Esc: back to the editor, unchanged. (Ctrl-Q still quits ted from the editor as today.)
- **Empty state.** If nothing changed since the baseline, say so.

## Goals
- Git-based, using the `git` command line (no libgit2 dependency).
- Logic testable without a terminal, per the project rules: git invocation, parsing, ordering, and the viewer state machine live in the library; `main.rs` only draws.
- No configuration.

## Non-goals
- Editing inside the git view (Enter hands off to the editor).
- Watching the filesystem with events; polling is fine.
- Staging, committing, or any git write operation.
- Non-git directories beyond a clear error message.
- Syntax highlighting, side-by-side diffs, word-level diffs, mouse support.

## Anti-goals
- It must never modify the repo or working tree.
- It must not flicker or lose scroll position on each refresh.
- A failing `git` call (not a repo, git missing) must show an error, not an empty list that looks like "no changes". Same spirit as the rule that a failed read is never treated as empty.

## Constraints
- Follow `CLAUDE.md`: TDD, logic in the library crate, `main.rs` thin, `cargo test` and `cargo clippy --all-targets` clean, pty smoke test extended to cover the viewer.
- Existing `ted <file>` behavior is unchanged.
- Rust, crossterm, existing dependencies; avoid adding crates unless clearly needed.

## Decisions by the user
- Git-based, not filesystem-mtime-based.
- Baseline = commit at open, resettable to now, or chosen from a list of recent commits (not a typed time like "30 minutes ago").
- `ted <directory>` opens the editor with no file, located in that directory; a key opens the git view from there (user's correction, replacing my earlier guess that the directory opens the viewer directly).
- Enter on a file opens it in the editor.
- Two panes: file list left, diff right.

## My calls (flagged for review)
- **App structure:** the library gets a top-level `App` that owns the `Editor` and an optional git view, with a mode (editor or git view). Ctrl-G enters the git view and `q`/Esc leaves it; the editor keeps its state. Enter on a file replaces the editor's buffer (see Keys). The git view does not poll while the editor is showing and refreshes when re-entered. The unsaved-changes quit confirm is unchanged. `main.rs` stays a thin draw/read-keys layer.
- **Scope:** the git view finds the repo with git, but lists only changes under the location directory. Paths are shown relative to it. Untracked files respect `.gitignore`. A location that is not in a git repo shows an error in the git view; the editor itself still works.
- **Parsing:** rename detection off (`--no-renames`), so a rename shows as `D` plus `A`. Binary files show a "binary file" placeholder instead of a diff. Very large files and diffs are truncated with a note.
- **Baseline edge cases:** a repo with no commits uses git's empty tree as the baseline, so everything shows as added. If the remembered baseline commit disappears (rebase, reset), show an error in the status line and fall back to the current `HEAD`.
- **Polling:** about once a second on the main loop, behind a small `Git` trait so tests use a fake. Only the selected file's diff is computed; the file list is computed every poll. Redraw only when the data changed. Keep the diff scroll position unless the selected file's diff changed.
- **Age:** file mtime as is. Deleted files use the time they were first noticed, which resets when the viewer restarts. Resetting the baseline does not clear it.
- **Keys:** Up/Down to select, PageUp/PageDown to scroll the diff, Enter to edit, `r` to reset the baseline to now, `b` to open the commit picker (Enter picks, Esc closes it), `q` or Esc to return to the editor; Ctrl-G opens the git view from the editor.
- Age display, exact pane widths, and the diff coloring (green `+`, red `-`) are details I'd settle while building.

## Left to the builder
Module structure, how diff text is parsed and styled (e.g. coloring `+`/`-` lines), pane widths, how many commits the picker lists, and how loading a file from the git view reuses the existing `Buffer`/`Editor`.
