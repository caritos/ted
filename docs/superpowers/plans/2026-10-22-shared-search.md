# Shared Search Term Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The remembered search term (Ctrl-F prefill, F3) is one per `App`, not per buffer: a term accepted in one buffer is what Ctrl-F prefills and F3 finds in every other buffer, and it survives closing/reloading buffers.

**Architecture:** `App` owns `search: String`. `Editor` keeps its `last_search` as a working copy: `App` pushes the shared term into the current editor before every key it handles in editor mode (`Editor::set_last_search`), and takes a newly accepted term out with `Editor::take_search() -> Option<String>` (set when a Find prompt is accepted with Enter and non-empty) in the same place it already takes commands (`finish_key`). No change to `Buffer::find` or the prompt itself.

**Tech Stack:** Rust, existing crates only.

**Spec:** none; design calls below (user may veto).

## Global Constraints

- TDD: failing test first. Logic in the library; `src/main.rs` untouched.
- Commands: `cargo test`; `cargo clippy --all-targets`; `cargo build --release && python3 -I scripts/pty_smoke.py target/release/ted`. If cargo is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Commit trailers: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01GuWFB6MMHvanjZKQigHDiM`.

## Design calls

- The shared term changes only when a Find prompt is accepted with Enter and a non-empty term (as today for the per-buffer term). Esc and not-found-then-Enter rules stay as they are today (`Enter` with a not-found term still remembers it, as now).
- A term remembered before a buffer is closed, reloaded or created stays; a brand-new `App` starts with an empty term.
- The message `No previous search` shows when the shared term is empty.
- Out of scope: highlighting matches, search history, per-buffer overrides.

## Review Focus

- Accept a term in buffer A, switch to B (Ctrl-N, buffer list, open from tree), Ctrl-F prefills it and F3 finds it in B.
- Close the buffer that set the term, reload an unmodified buffer from disk: the term survives.
- Esc in the prompt, an empty Enter and keys in views/help do not change the term; a prompt open while switching is impossible, but a key routed through the open-prompt branch must not clobber the term with a stale copy.
- Two buffers with different cursors: F3 starts from each buffer's own cursor.

---

### Task 1: One search term in `App`

**Files:** Modify `src/editor.rs` (`set_last_search`, `take_search`, mark a newly accepted term), `src/app.rs` (field `search`, push before keys in editor mode, take in the same place commands are taken, both prompt-open and normal paths), tests in `src/app/tests/search.rs` (extend) and `src/editor/prompt_tests.rs`, `scripts/pty_smoke.py`, `README.md`, `CLAUDE.md` (search line, dated log entry: done / not hand-verified / deferred; the old "Deferred: one search term shared across buffers" lines must be updated).

**Interfaces:** Produces `Editor::set_last_search(&mut self, term: &str)` and `Editor::take_search(&mut self) -> Option<String>`; `App.search: String` (private).

- [ ] **Step 1: Failing tests:** App: accept `foo` in buffer A, Ctrl-N to B, Ctrl-F shows `Find: foo_` prefilled and Enter accepts, F3 in B jumps to `foo` from B's own cursor; same after switching with the buffer list and after opening a second file from the tree; close A (Ctrl-W) then Ctrl-F in B still prefilled; reloading an unmodified buffer from the tree keeps the term; Esc keeps the previous term; accepting a new term in B makes A prefill the new term; empty term + F3 gives `No previous search` in every buffer; the term is not changed by keys in the git view/tree/help. Editor: `take_search` returns the term once after Enter (None after Esc / empty Enter), `set_last_search` makes Ctrl-F prefill and F3 use it.
- [ ] **Step 2:** `cargo test` — confirm failures are for the right reason.
- [ ] **Step 3:** Implement. Push the shared term at the start of `App::handle_key` when the mode is the editor; pull via `take_search` wherever `take_command` is taken.
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets`.
- [ ] **Step 5:** pty smoke: two files `a.txt` / `b.txt` opened via the tree (as the multi-buffer scenario does); Ctrl-F, `needle`, Enter in the first; Ctrl-N; Ctrl-F shows `Find: needle` prefilled in the second; Esc; F3 moves the cursor to `needle` in the second (status row:col). Show it failing against `~/.cargo/bin/ted` (no sharing), then passing on the release build.
- [ ] **Step 6:** Docs; commit `Search: one remembered term for all buffers`.
