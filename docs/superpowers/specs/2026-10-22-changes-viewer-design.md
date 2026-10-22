# Changes viewer (`ted <directory>`)

## Intent
I program with Claude and want to watch what it changes without leaving my editor. Running `ted <directory>` opens a read-only viewer: a live list of files that changed in the git repo, and the diff of the selected file. Today `ted <file>` opens the editor; a directory argument is the new case.

Success: I run it beside a Claude session and, at a glance, see which files are changing and what changed, even after Claude has committed them.

## Behavior
- **Baseline.** On open, remember the current `HEAD` commit. "Changed" means any file that differs between that commit and the working tree. Commits Claude makes after the baseline do not hide changes.
- **Left pane: file list.** One row per changed file: status letter (`M` modified, `A` added, `D` deleted, `?` untracked), path, and age ("2s", "14s", "3m"). Sorted newest first. Age comes from the file's modified time (the builder's call; git has no such data). Deleted files have no mtime and sort by the time they were first noticed.
- **Right pane: diff.** The unified diff of the selected file against the baseline, read-only, scrollable. Untracked files show as all-added.
- **Live.** The list and diff refresh on their own about once a second. No keypress needed. The selection stays on the same file when the list reorders (key by path, not row position).
- **Keys.**
  - Up/Down: move selection in the list.
  - PageUp/PageDown (or similar): scroll the diff.
  - Enter: open the selected file in the normal editor. Quitting that editor returns to the viewer (builder's call on the exact flow).
  - A key to reset the baseline to now: baseline becomes the current `HEAD`. (Uncommitted edits already in the working tree still show, since they differ from `HEAD`.)
  - A key to open a list of recent commits; choosing one makes it the baseline. Modeled on the commit list in IntelliJ's Git Log (user's reference screenshot): one row per commit, newest first, showing short hash, subject, date and time, and branch/tag labels next to the subject (e.g. `main`, `origin/main`, `swift-mac-app`). No author column (my call: nearly every commit here is by the same person); no graph lines.
  - `q` / Ctrl-Q: quit.
- **Empty state.** If nothing changed since the baseline, say so.

## Goals
- Git-based, using the `git` command line (no libgit2 dependency).
- Logic testable without a terminal, per the project rules: git invocation, parsing, ordering, and the viewer state machine live in the library; `main.rs` only draws.
- No configuration.

## Non-goals
- Editing inside the viewer (Enter hands off to the editor).
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
- `ted <directory>` opens it.
- Enter on a file opens it in the editor.
- Two panes: file list left, diff right.

## My calls (flagged for review)
- **App structure:** the library gets a top-level `App` with a mode (viewer or editor). Enter switches to an `Editor` for that file; quitting it (Ctrl-Q, with the existing unsaved-changes confirm unchanged) returns to the viewer. The viewer does not poll while the editor is open and refreshes on return. `main.rs` stays a thin draw/read-keys layer.
- **Scope:** `ted <dir>` finds the repo with git, but lists only changes under `<dir>`. Paths are shown relative to `<dir>`. Untracked files respect `.gitignore`. A `<dir>` that is not in a git repo shows an error.
- **Parsing:** rename detection off (`--no-renames`), so a rename shows as `D` plus `A`. Binary files show a "binary file" placeholder instead of a diff. Very large files and diffs are truncated with a note.
- **Baseline edge cases:** a repo with no commits uses git's empty tree as the baseline, so everything shows as added. If the remembered baseline commit disappears (rebase, reset), show an error in the status line and fall back to the current `HEAD`.
- **Polling:** about once a second on the main loop, behind a small `Git` trait so tests use a fake. Only the selected file's diff is computed; the file list is computed every poll. Redraw only when the data changed. Keep the diff scroll position unless the selected file's diff changed.
- **Age:** file mtime as is. Deleted files use the time they were first noticed, which resets when the viewer restarts. Resetting the baseline does not clear it.
- **Keys:** Up/Down to select, PageUp/PageDown to scroll the diff, Enter to edit, `r` to reset the baseline to now, `b` to open the commit picker (Enter picks, Esc closes it), `q` or Ctrl-Q to quit the viewer.
- Age display, exact pane widths, and the diff coloring (green `+`, red `-`) are details I'd settle while building.

## Left to the builder
Module structure, how diff text is parsed and styled (e.g. coloring `+`/`-` lines), pane widths, how many commits the picker lists, and how the Enter hand-off reuses the existing `Editor`.
