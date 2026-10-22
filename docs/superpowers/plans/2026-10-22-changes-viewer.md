# Git Changes View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ted <dir>` opens the editor on an empty buffer located in that directory, and Ctrl-G opens a live, read-only git view (changed-file list plus diff against a baseline commit).

**Architecture:** A new library layer talks to the `git` CLI behind a `Git` trait (fakeable). A `GitView` state machine owns the baseline, file list, selection, diff scroll and commit picker. A top-level `App` owns the `Editor` plus an optional `GitView`, switches modes, and produces a styled `Screen`; `main.rs` only reads keys (with a poll timeout for live refresh) and draws the `Screen`.

**Tech Stack:** Rust 2024, crossterm 0.29, ropey (existing). No new crates. Shell out to `git`.

**Spec:** `docs/superpowers/specs/2026-10-22-changes-viewer-design.md`

## Global Constraints

- TDD: failing test first; logic in the library crate, `main.rs` stays a thin draw/read-keys layer.
- Fast tier: `cargo test`. Lint: `cargo clippy --all-targets` must be clean. Real-terminal check: `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`.
- If `cargo` is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Use the `git` command line only (no libgit2); read-only git commands only. Never modify the repo or working tree.
- No new dependencies.
- `ted <file>` behavior is unchanged. A missing file still opens as an empty buffer; any other read error is an error. Never overwrite on a failed read.
- A failed git call is shown as an error, never as an empty list that looks like "no changes".
- Keys: Ctrl-G opens the git view from the editor; in the git view Up/Down select, PageUp/PageDown scroll the diff, Enter opens the file, `r` resets baseline to now (current `HEAD`), `b` opens the commit picker (Up/Down move, Enter picks, Esc closes), `q`/Esc returns to the editor.
- No author column and no graph in the commit picker; rows show short hash, subject, date and time, and branch/tag labels.
- Untracked files respect `.gitignore`. Rename detection off (`--no-renames`).
- Poll about once a second.
- Update `README.md` and the dated log in `CLAUDE.md` when done.

## Review Focus

Inputs the spec implies but no obvious test would cover; each has a pinning test in the owning task.

1. File names with spaces, unicode or newlines in git output (Task 1: parsing uses `-z`).
2. Location is a subdirectory of the repo: only changes under it, paths relative to it (Task 2).
3. Location is not a git repo: git view shows an error, editor still works (Tasks 2, 5).
4. Enter on a deleted file must not open an empty buffer that a later save would recreate (Task 3).
5. Tiny terminals (width or height of 0 to 5) must not panic in any render (Tasks 4, 5).

## File Structure

- `src/git.rs` (create): types, parsers, `Git` trait, `GitCli`.
- `src/git/tests.rs` (create): parser tests and `GitCli` tests against real temp repos.
- `src/screen.rs` (create): `Style`, `Span`, `Row`, `Screen` (shared by editor and git view rendering).
- `src/gitview.rs` (create): `GitView` state machine and rendering. Tests in `src/gitview/tests.rs` with a `FakeGit`.
- `src/app.rs` (create): `App`, mode switching, editor/git hand-off. Tests in `src/app/tests.rs`.
- `src/editor.rs` (modify): add `Editor::load(&mut self, Buffer)`.
- `src/lib.rs` (modify): declare the new modules.
- `src/main.rs` (modify): argument handling, poll loop, draw `Screen` with styles.
- `scripts/pty_smoke.py` (modify): cover `ted <dir>` + Ctrl-G.

---

### Task 1: Git types and parsers

**Files:**
- Create: `src/git.rs`, `src/git/tests.rs`
- Modify: `src/lib.rs` (add `pub mod git;`)

**Interfaces:**
- Produces (all `pub` in `ted::git`):
  - `enum ChangeKind { Modified, Added, Deleted, Untracked }` with `fn letter(&self) -> char` (`M A D ?`). Derive `Debug, Clone, Copy, PartialEq, Eq`.
  - `struct Change { pub kind: ChangeKind, pub path: String }` (`Debug, Clone, PartialEq, Eq`).
  - `enum Baseline { Empty, Commit(String) }` (`Debug, Clone, PartialEq, Eq`) with `fn rev(&self) -> &str` (the empty-tree hash `4b825dc642cb6eb9a060e54bf8d69288fbee4904` for `Empty`).
  - `struct Commit { pub hash: String, pub short: String, pub subject: String, pub unix_time: i64, pub refs: Vec<String> }`.
  - `enum GitError { NotARepo, Failed(String) }` implementing `Display` and `std::error::Error`; `NotARepo` displays "not a git repository".
  - `fn parse_name_status_z(out: &str) -> Vec<Change>`: input from `git diff --name-status -z`: `M\0path\0A\0path\0`. Unknown status letters (e.g. `T`, `U`) map to `Modified`.
  - `fn parse_untracked_z(out: &str) -> Vec<Change>`: input from `git ls-files --others --exclude-standard -z`: `path\0path\0`, all `Untracked`.
  - `fn parse_log(out: &str) -> Vec<Commit>`: lines of `%H\t%h\t%ct\t%D\t%s`; refs from `%D` are comma-separated like `HEAD -> main, origin/main, tag: v1`, parsed into labels with the `HEAD -> ` prefix and `tag: ` prefix stripped; `HEAD` alone is dropped. Subject is everything after the fourth tab (may contain tabs). Malformed lines are skipped.
  - `fn format_age(secs: u64) -> String`: `0..60` -> `"{n}s"`, `<3600` -> `"{n}m"`, `<86400` -> `"{n}h"`, else `"{n}d"`.

- [ ] **Step 1: Write failing tests in `src/git/tests.rs`**
  - `name_status_parses_each_kind`: `"M\0a.rs\0A\0b.rs\0D\0c.rs\0"` gives `[Modified a.rs, Added b.rs, Deleted c.rs]`.
  - `name_status_keeps_spaces_unicode_and_newlines`: path `"my file\nwith é.rs"` round-trips exactly.
  - `name_status_unknown_letter_is_modified`: `"T\0x\0"`.
  - `name_status_empty_and_truncated_input`: `""` gives `[]`; `"M\0"` (status without a path) gives `[]`.
  - `untracked_parses_paths`: `"a\0b c\0"` gives two `Untracked` entries.
  - `log_parses_fields_and_refs`: a line `"H1\tabc\t1700000000\tHEAD -> main, origin/main, tag: v1\tAdd viewer"` gives short `abc`, refs `["main","origin/main","v1"]`, subject `"Add viewer"`.
  - `log_subject_with_tabs_and_empty_refs`: subject `"a\tb"` survives; empty `%D` gives `refs == []`.
  - `log_skips_malformed_lines`.
  - `empty_baseline_uses_empty_tree_hash`.
  - `format_age_boundaries`: 0 -> `0s`, 59 -> `59s`, 60 -> `1m`, 3599 -> `59m`, 3600 -> `1h`, 86399 -> `23h`, 86400 -> `1d`.
- [ ] **Step 2: Run `cargo test git::` and confirm FAIL** (module missing / functions undefined).
- [ ] **Step 3: Implement the types and parsers in `src/git.rs`** with the signatures above, plus `#[cfg(test)] mod tests;` at the bottom (same pattern as `editor.rs`).
- [ ] **Step 4: Run `cargo test git::` and `cargo clippy --all-targets`; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git add src/git.rs src/git src/lib.rs && git commit -m "Add git types and output parsers"`.

---

### Task 2: `Git` trait and `GitCli`

**Files:**
- Modify: `src/git.rs`, `src/git/tests.rs`

**Interfaces:**
- Consumes: Task 1 types.
- Produces:
  - `trait Git` with:
    - `fn head(&self) -> Result<Baseline, GitError>` (`Baseline::Empty` when the repo has no commits)
    - `fn baseline_exists(&self, base: &Baseline) -> bool`
    - `fn changes(&self, base: &Baseline) -> Result<Vec<Change>, GitError>` (tracked changes plus untracked files, only under the location, paths relative to it, no duplicates, no ordering guarantee)
    - `fn diff(&self, base: &Baseline, change: &Change) -> Result<String, GitError>` (unified diff text; capped at `MAX_DIFF_BYTES = 200_000` with a trailing line `"... diff truncated"`)
    - `fn log(&self, limit: usize) -> Result<Vec<Commit>, GitError>`
    - `fn modified(&self, path: &str) -> Option<std::time::SystemTime>` (mtime of the working-tree file; `None` if it does not exist)
  - `struct GitCli` with `pub fn new(dir: impl Into<PathBuf>) -> Self`, implementing `Git`. Every call runs `git -C <dir> ...` with `LC_ALL=C`. stderr containing `not a git repository` maps to `GitError::NotARepo`; any other failure, or git not installed, maps to `Failed(message)`.

- [ ] **Step 1: Write failing tests** using a helper in `src/git/tests.rs` that makes a unique temp dir (same pattern as `temp_file` in `src/editor/tests.rs`), runs `git init`, and sets `user.name`/`user.email` locally so commits work offline.
  - `head_of_repo_without_commits_is_empty`
  - `head_returns_current_commit_after_commit`
  - `changes_reports_modified_added_deleted_untracked`: commit `a`, `b`; modify `a`, delete `b`, `git add` a new `c`, create untracked `d`.
  - `changes_respects_gitignore`
  - `changes_with_no_commits_uses_empty_baseline`: all files show as added/untracked, no error.
  - `changes_in_subdirectory_are_scoped_and_relative`: `GitCli::new(repo/sub)`; a change in `repo/other.txt` is absent; a change in `repo/sub/x.txt` is reported as `x.txt`.
  - `changes_include_commits_made_after_baseline`: record `head()`, commit another edit, `changes(&base)` still lists it.
  - `changes_with_odd_file_names`: a name with a space and one with unicode.
  - `diff_of_modified_file_has_hunk_and_plus_minus_lines`
  - `diff_of_untracked_file_shows_all_added` (uses `git diff --no-index -- /dev/null <path>`; exit status 1 is success).
  - `diff_of_binary_file_says_binary` (contains `Binary`).
  - `diff_is_truncated_over_cap`: file larger than the cap ends with `... diff truncated`.
  - `log_lists_recent_commits_newest_first_with_refs` (limit honored).
  - `not_a_repo_is_reported`: a plain temp dir gives `Err(GitError::NotARepo)` from `head()` and `changes()`.
  - `baseline_exists_false_for_unknown_hash`.
  - `modified_returns_none_for_missing_file`.
- [ ] **Step 2: Run `cargo test git::` and confirm FAIL.**
- [ ] **Step 3: Implement `Git` and `GitCli`.** Commands: `rev-parse --verify HEAD` (failure with "Needed a single revision" or similar and no commits means `Empty`; distinguish from not-a-repo by checking `rev-parse --git-dir` first); `diff --name-status -z --no-renames <rev> -- .`; `ls-files --others --exclude-standard -z`; `diff --no-color --no-renames <rev> -- <path>` for tracked changes; `log -n <limit> --date=unix --format=%H%x09%h%x09%ct%x09%D%x09%s`; `cat-file -e <rev>^{commit}` for `baseline_exists`. Run with the location as cwd so pathspec `.` scopes to it, and pass `--relative` to `diff` so paths come back relative to the location.
- [ ] **Step 4: Run tests and clippy; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Add Git trait and git CLI implementation"`.

---

### Task 3: `Screen` types and `GitView` state machine

**Files:**
- Create: `src/screen.rs`, `src/gitview.rs`, `src/gitview/tests.rs`, `src/git/fake.rs`
- Modify: `src/lib.rs` (add `pub mod screen; pub mod gitview;`)

**Interfaces:**
- Consumes: `Git`, `Change`, `ChangeKind`, `Baseline`, `Commit`, `GitError`, `format_age` from Task 1-2.
- Produces in `ted::screen`:
  - `enum Style { Normal, Selected, Added, Removed, Hunk, Dim }`
  - `struct Span { pub text: String, pub style: Style }`, `type Row = Vec<Span>`
  - `struct Screen { pub rows: Vec<Row>, pub status: String, pub message: String, pub cursor: Option<(usize, usize)> }`. All derive `Debug, Clone, PartialEq, Eq`.
- Produces in `ted::gitview`:
  - `enum GitAction { None, Back, Open(String) }` (`Open` carries a location-relative path)
  - `struct GitView` with `pub fn new() -> Self`, `pub fn refresh(&mut self, git: &dyn Git, now: SystemTime)`, `pub fn handle_key(&mut self, key: KeyEvent, git: &dyn Git, now: SystemTime) -> GitAction`, `pub fn selected_path(&self) -> Option<&str>`, `pub fn baseline(&self) -> Option<&Baseline>`, `pub fn error(&self) -> Option<&str>`, `pub fn message(&self) -> &str`.
  - `refresh` behavior: the first call sets the baseline to `git.head()`. Each call rebuilds the file list from `git.changes()`, sorted newest first by `git.modified()` (files with no mtime, i.e. deleted, use the time they were first noticed, tracked in a path-to-time map that is not cleared by `r`), tie-broken by path. The selection follows the path, not the row; if the selected path disappears, select the first row. The diff is fetched only for the selected file and the scroll offset is kept unless that file's diff text changed. If `baseline_exists` is false, set `message` to "Baseline commit is gone; reset to HEAD" and reset to `git.head()`. A git error sets `error` (shown instead of the list) and keeps the last good state hidden, never showing an empty list as "no changes".

- [ ] **Step 1: Write failing tests** in `src/gitview/tests.rs` with a `FakeGit` defined in `src/git/fake.rs` (`#[cfg(test)] pub(crate) mod fake;` in `src/git.rs`, reused by the app tests; fields: head, changes, diffs, log, mtimes, `fail: Option<GitError>`, `existing: bool`), covering:
  - `first_refresh_sets_baseline_to_head`
  - `list_sorted_newest_first`
  - `selection_follows_path_when_list_reorders`
  - `selection_falls_back_to_first_when_selected_file_disappears`
  - `empty_changes_gives_empty_list_without_error`
  - `git_error_sets_error_and_not_empty_state` (NotARepo text is "not a git repository")
  - `deleted_file_sorts_by_first_noticed_time_and_keeps_it_after_reset` 
  - `diff_scroll_kept_when_diff_unchanged_and_reset_when_it_changes`
  - `page_down_and_up_scroll_diff_never_below_zero`
  - `up_down_move_selection_and_clamp_at_ends`
  - `r_resets_baseline_to_head`
  - `b_opens_picker_with_log_rows_and_enter_picks_baseline`
  - `esc_closes_picker_without_changing_baseline`
  - `empty_log_picker_shows_message_and_closes_on_esc`
  - `vanished_baseline_falls_back_to_head_with_message`
  - `enter_on_file_returns_open_with_path`
  - `enter_on_deleted_file_does_nothing_and_sets_message` (Review Focus 4)
  - `enter_with_empty_list_does_nothing`
  - `q_and_esc_return_back`
  - `keys_with_no_selection_do_not_panic`
- [ ] **Step 2: Run `cargo test gitview::` and confirm FAIL.**
- [ ] **Step 3: Implement `src/screen.rs` and the `GitView` state machine** (no rendering yet) with the signatures above. Picker state is private; expose what tests need via `pub(crate)` accessors if required.
- [ ] **Step 4: Run tests and clippy; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Add Screen types and GitView state machine"`.

---

### Task 4: Render `GitView` to a `Screen`

**Files:**
- Modify: `src/gitview.rs`, `src/gitview/tests.rs`

**Interfaces:**
- Consumes: Task 3.
- Produces: `GitView::render(&self, width: usize, height: usize, now: SystemTime) -> Screen`. `height` includes the status and message rows (like `Editor::view`).
  - Layout: left pane about one third of the width (at least 12 columns, builder's call), a one-column separator, right pane the rest. Left rows: `"{letter} {path}  {age}"`, selected row styled `Selected`, path clipped to fit. Right rows: the diff from the scroll offset; `+` lines `Added`, `-` lines `Removed`, `@@` lines `Hunk`, `diff`/`index`/`---`/`+++` header lines `Dim`, others `Normal`.
  - Status row: baseline short description (e.g. `Baseline abc1234 "subject"` or `Baseline: (no commits)`) and change count. Message row: `message()`.
  - Error state: the first row shows the error text; no list.
  - Empty state: left pane shows "No changes since baseline".
  - Picker open: replaces the whole body with one row per commit: `"{short}  {subject}  {YYYY-MM-DD HH:MM}  [{refs joined by ", "}]"`, selected row `Selected`. Date formatting is UTC and done by hand (no new crates); a small pure `fn format_commit_time(unix: i64) -> String` is unit-tested.
  - `cursor` is always `None`. Every row is clipped to `width` by characters.

- [ ] **Step 1: Write failing tests:**
  - `render_shows_status_letter_path_and_age`
  - `render_styles_diff_lines`
  - `render_selected_row_is_styled_selected`
  - `render_empty_state_text`
  - `render_error_state_text`
  - `render_picker_rows_contain_short_hash_subject_time_and_refs`
  - `format_commit_time_known_values` (e.g. `1700000000` -> `"2023-11-14 22:13"`, `0` -> `"1970-01-01 00:00"`)
  - `render_never_exceeds_width_or_height`, looped over widths 0..=40 and heights 0..=8, with a list, an error, and the picker (Review Focus 5).
  - `render_with_unicode_paths_clips_by_char_not_byte`
- [ ] **Step 2: Run `cargo test gitview::` and confirm FAIL.**
- [ ] **Step 3: Implement `render` and `format_commit_time`.**
- [ ] **Step 4: Run tests and clippy; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Render the git view to a Screen"`.

---

### Task 5: `App` mode switching

**Files:**
- Create: `src/app.rs`, `src/app/tests.rs`
- Modify: `src/editor.rs` (add `pub fn load(&mut self, buffer: Buffer)` which replaces the buffer and resets scroll and the armed-quit flag), `src/editor/tests.rs` (test for `load`), `src/lib.rs` (add `pub mod app;`)

**Interfaces:**
- Consumes: `Editor`, `Flow`, `View`, `GitView`, `GitAction`, `Git`, `Screen`, `Buffer`.
- Produces in `ted::app`:
  - `struct App` with `pub fn new(editor: Editor, location: PathBuf, git: Box<dyn Git>) -> Self`.
  - `pub fn handle_key(&mut self, key: KeyEvent, now: SystemTime) -> Flow`
  - `pub fn tick(&mut self, now: SystemTime)` (refreshes the git view only while it is showing)
  - `pub fn screen(&mut self, width: usize, height: usize, now: SystemTime) -> Screen`
  - `pub fn in_git_view(&self) -> bool`, `pub fn editor(&self) -> &Editor`
  - Behavior: Ctrl-G in the editor creates the `GitView` on first use, refreshes it, and switches mode; later Ctrl-G refreshes and switches. `GitAction::Back` returns to the editor with buffer and cursor untouched. `GitAction::Open(rel)`: if the editor's buffer `is_modified()`, stay in the git view and set the git view message "Unsaved changes: save or quit first" (via a `GitView::set_message(&mut self, String)`); otherwise load `Buffer::open(location.join(rel))` into the editor and return to the editor; on error show the message "Open failed: {e}" and stay. In the git view, Ctrl-Q is passed to the editor's `handle_key` so quitting and the unsaved confirm behave as today. The editor screen is `Editor::view` converted to a `Screen` (one `Normal` span per row, cursor `Some`).

- [ ] **Step 1: Write failing tests** using the `FakeGit` from `src/git/fake.rs`:
  - `editor_load_replaces_buffer_and_resets_scroll` (in `src/editor/tests.rs`)
  - `ctrl_g_switches_to_git_view_and_q_returns_with_editor_state_intact`
  - `typing_in_editor_is_unaffected_before_and_after_git_view`
  - `enter_loads_selected_file_into_editor`
  - `enter_with_unsaved_changes_refuses_and_keeps_buffer`
  - `enter_on_unreadable_file_shows_error_and_stays_in_git_view`
  - `ctrl_q_in_git_view_follows_editor_quit_rules` (clean buffer quits; modified needs two presses)
  - `not_a_repo_location_shows_error_in_git_view_and_editor_still_works`
  - `tick_refreshes_only_in_git_view`
  - `screen_of_editor_matches_editor_view_text`
  - `screen_never_panics_for_tiny_sizes` (widths 0..=5, heights 0..=5, both modes)
- [ ] **Step 2: Run `cargo test app::` and confirm FAIL.**
- [ ] **Step 3: Implement `Editor::load`, `App`, and the `View` to `Screen` conversion.**
- [ ] **Step 4: Run the full `cargo test` and `cargo clippy --all-targets`; confirm PASS and clean.**
- [ ] **Step 5: Commit** `git commit -m "Add App with editor and git view modes"`.

---

### Task 6: Wire up `main.rs`

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `App`, `Screen`, `GitCli`.
- Produces: no new public API.

- [ ] **Step 1: Argument handling.** No argument: `Buffer::new()`, location `.`. A path that is a directory: `Buffer::new()`, location is that directory. Any other path: `Buffer::open(path)` as today, location is the path's parent (`.` if it has none). Check `is_dir()` before `Buffer::open`, because opening a directory is a read error today. Keep the existing usage message and errors. Verify by hand after the build: `ted .` opens an empty `[No Name]` buffer, and `ted nonexistent.txt` still opens an empty named buffer.
- [ ] **Step 2: Loop.** Replace the blocking `event::read()` with `event::poll(Duration::from_millis(1000))`. On timeout call `app.tick(now)`. Build the `Screen` each iteration and skip drawing when it equals the previously drawn `Screen` and the terminal size is unchanged. Key handling still ignores `KeyEventKind::Release`. Quit when `app.handle_key` returns `Flow::Quit`.
- [ ] **Step 3: Draw `Screen`.** Draw each `Span` with its style: `Selected` reverse video, `Added` green, `Removed` red, `Hunk` cyan, `Dim` dim attribute, `Normal` plain; reset attributes after each span, clear to end of line. Status row stays reverse video. Hide the cursor when `Screen::cursor` is `None`.
- [ ] **Step 4: Run `cargo test`, `cargo clippy --all-targets`, `cargo build --release`; confirm all pass.**
- [ ] **Step 5: Commit** `git commit -m "Wire App into the terminal layer"`.

---

### Task 7: pty smoke test, docs, and hand verification

**Files:**
- Modify: `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md`

- [ ] **Step 1: Extend `scripts/pty_smoke.py`** with a scenario: create a temp git repo with one commit, then an uncommitted edit to a tracked file and a new untracked file; spawn `ted <repo>`; send Ctrl-G; assert the output contains both file names and a `+` diff line; send `q`; type a character and assert the status shows `[+]`; Ctrl-Q twice exits with status 0. Second scenario: `ted <non-repo temp dir>`, Ctrl-G shows "not a git repository", `q` returns, Ctrl-Q quits. Run it and see it fail before the implementation exists (it passes now that Tasks 1-6 are done; confirm it fails if Ctrl-G handling is stubbed out, to prove the test is real).
- [ ] **Step 2: Run the full suite on a clean build:** `cargo clean && cargo test && cargo clippy --all-targets && cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. All green.
- [ ] **Step 3: Update `README.md`** (usage of `ted <dir>`, Ctrl-G and the git view keys) and the dated `CLAUDE.md` log: Done (unit-tested + pty smoke) / Not yet verified by hand: the live refresh while Claude edits, resize in the git view, a very large repo / Deferred: wide-char alignment in the git view, mouse, word diffs.
- [ ] **Step 4: Hand verification checklist for the user** (not claimed done until they confirm): run `ted .` in a repo while Claude edits files; Ctrl-G shows changes updating within about a second; `r` and `b` change the baseline; Enter opens a file; `q` returns.
- [ ] **Step 5: Commit** `git commit -m "Cover git view in pty smoke test; update docs"`.
