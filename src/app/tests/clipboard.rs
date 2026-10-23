use super::*;
use crate::clipboard::{ClipboardRead, MemoryClipboard};

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

/// An app on `text` with a shared in-memory clipboard.
fn clip_app(text: &str) -> (App, MemoryClipboard) {
    let clip = MemoryClipboard::new();
    let app = App::with_clipboard(
        Editor::new(Buffer::from_text(text)),
        PathBuf::from("/nowhere"),
        Box::new(FakeGit::new()),
        Box::new(clip.clone()),
    );
    (app, clip)
}

fn press(app: &mut App, ev: KeyEvent, times: usize) {
    for _ in 0..times {
        app.handle_key(ev, t(1000));
    }
}

fn text(app: &App) -> String {
    app.editor().buffer().text()
}

fn message(app: &App) -> String {
    app.editor().message().to_string()
}

fn run(app: &mut App, command: &str) {
    app.handle_key(ctrl('x'), t(1000));
    type_str(app, command);
    app.handle_key(key(KeyCode::Enter), t(1000));
}

fn undo(app: &mut App) {
    app.handle_key(ctrl('z'), t(1000));
}

// ---- copy ----

#[test]
fn ctrl_c_copies_the_selection_to_the_clipboard_and_leaves_the_text() {
    let (mut app, clip) = clip_app("hello world");
    press(&mut app, shift(KeyCode::Right), 5);
    app.handle_key(ctrl('c'), t(1000));
    assert_eq!(clip.contents().as_deref(), Some("hello"));
    assert_eq!(text(&app), "hello world");
    assert!(!app.editor().buffer().is_modified());
    assert_eq!(message(&app), "Copied 5 chars");
    // The selection stays, so Ctrl-K can follow.
    assert_eq!(app.editor().buffer().selected_text().as_deref(), Some("hello"));
}

#[test]
fn copy_counts_chars_not_bytes_and_spans_lines() {
    let (mut app, clip) = clip_app("héé\nx");
    app.handle_key(ctrl('a'), t(1000));
    app.handle_key(ctrl('c'), t(1000));
    assert_eq!(clip.contents().as_deref(), Some("héé\nx"));
    assert_eq!(message(&app), "Copied 5 chars");
}

#[test]
fn copy_and_cut_without_a_selection_say_nothing_selected() {
    for combo in [ctrl('c'), ctrl('k')] {
        let (mut app, clip) = clip_app("abc");
        clip.set_contents(Some("old"));
        app.handle_key(combo, t(1000));
        assert_eq!(message(&app), "Nothing selected");
        assert_eq!(text(&app), "abc");
        assert!(!app.editor().buffer().is_modified());
        assert_eq!(clip.contents().as_deref(), Some("old"), "the clipboard is untouched");
    }
}

#[test]
fn copy_with_a_broken_clipboard_still_fills_the_register() {
    let (mut app, clip) = clip_app("abc def");
    clip.set_broken(true);
    press(&mut app, shift(KeyCode::Right), 3);
    app.handle_key(ctrl('c'), t(1000));
    assert_eq!(message(&app), "Copied 3 chars (ted only)");
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "abc defabc");
}

#[test]
fn copy_and_cut_say_ted_only_when_the_system_clipboard_was_not_updated() {
    let (mut app, clip) = clip_app("abc def");
    clip.set_writes_fail(true);
    press(&mut app, shift(KeyCode::Right), 3);
    app.handle_key(ctrl('c'), t(1000));
    assert_eq!(message(&app), "Copied 3 chars (ted only)");
    app.handle_key(ctrl('k'), t(1000));
    assert_eq!(message(&app), "Cut 3 chars (ted only)");
    assert_eq!(text(&app), " def");
    assert_eq!(clip.contents(), None);
    clip.set_writes_fail(false);
    app.handle_key(ctrl('a'), t(1000));
    app.handle_key(ctrl('c'), t(1000));
    assert_eq!(message(&app), "Copied 4 chars");
}

// ---- cut ----

#[test]
fn ctrl_k_cuts_and_one_undo_restores() {
    let (mut app, clip) = clip_app("hello world");
    press(&mut app, shift(KeyCode::Right), 6);
    app.handle_key(ctrl('k'), t(1000));
    assert_eq!(text(&app), "world");
    assert_eq!(clip.contents().as_deref(), Some("hello "));
    assert_eq!(message(&app), "Cut 6 chars");
    assert!(app.editor().buffer().is_modified());
    assert!(app.editor().buffer().selection().is_none());
    undo(&mut app);
    assert_eq!(text(&app), "hello world");
    assert!(!app.editor().buffer().is_modified());
    assert_eq!(clip.contents().as_deref(), Some("hello "), "the clipboard keeps the text");
}

#[test]
fn select_all_cut_empties_the_buffer_and_paste_brings_it_back() {
    let (mut app, _clip) = clip_app("one\ntwo\n");
    app.handle_key(ctrl('a'), t(1000));
    app.handle_key(ctrl('k'), t(1000));
    assert_eq!(text(&app), "");
    assert!(app.editor().buffer().is_modified());
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "one\ntwo\n");
    assert_eq!(app.editor().buffer().cursor(), (2, 0));
}

// ---- paste ----

#[test]
fn ctrl_v_inserts_at_the_cursor_as_one_undo_step() {
    let (mut app, clip) = clip_app("ad");
    clip.set_contents(Some("b\nc"));
    app.handle_key(key(KeyCode::Right), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab\ncd");
    assert_eq!(app.editor().buffer().cursor(), (1, 1));
    assert_eq!(message(&app), "");
    undo(&mut app);
    assert_eq!(text(&app), "ad");
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn paste_replaces_a_selection_as_one_undo_step() {
    let (mut app, clip) = clip_app("one two three");
    clip.set_contents(Some("2"));
    app.handle_key(key(KeyCode::End), t(1000));
    press(&mut app, shift(KeyCode::Left), 5);
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "one two 2");
    undo(&mut app);
    assert_eq!(text(&app), "one two three");
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn paste_uses_the_register_only_when_no_clipboard_tool_exists() {
    let (mut app, clip) = clip_app("ab");
    press(&mut app, shift(KeyCode::Right), 1);
    app.handle_key(ctrl('c'), t(1000));
    clip.set_broken(true);
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "aba");
}

#[test]
fn paste_does_not_use_a_stale_register_when_the_clipboard_has_no_text() {
    // Copied in ted, then an image or file was copied elsewhere.
    for how in ["none", "empty"] {
        let (mut app, clip) = clip_app("ab");
        press(&mut app, shift(KeyCode::Right), 1);
        app.handle_key(ctrl('c'), t(1000));
        clip.set_contents(if how == "none" { None } else { Some("") });
        app.handle_key(key(KeyCode::End), t(1000));
        app.handle_key(ctrl('v'), t(1000));
        assert_eq!(text(&app), "ab", "{how}");
        assert_eq!(message(&app), "Clipboard has no text", "{how}");
        assert!(!app.editor().buffer().is_modified());
    }
}

#[test]
fn paste_does_not_use_a_stale_register_when_reading_failed() {
    let (mut app, clip) = clip_app("ab");
    press(&mut app, shift(KeyCode::Right), 1);
    app.handle_key(ctrl('c'), t(1000));
    clip.set_forced_read(Some(ClipboardRead::Failed));
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab");
    assert_eq!(message(&app), "Clipboard read failed");
}

#[test]
fn paste_refuses_a_clipboard_over_the_size_limit() {
    let (mut app, clip) = clip_app("ab");
    clip.set_forced_read(Some(ClipboardRead::TooLarge));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab");
    assert_eq!(message(&app), "Clipboard too large");
}

#[test]
fn paste_uses_the_register_after_a_copy_the_system_never_got() {
    for read in [None, Some(ClipboardRead::Failed)] {
        let (mut app, clip) = clip_app("ab");
        clip.set_contents(Some("old"));
        clip.set_writes_fail(true);
        press(&mut app, shift(KeyCode::Right), 1);
        app.handle_key(ctrl('c'), t(1000));
        // The system still holds the older text, or cannot be read at all.
        clip.set_forced_read(read.clone());
        app.handle_key(key(KeyCode::End), t(1000));
        app.handle_key(ctrl('v'), t(1000));
        if read.is_some() {
            assert_eq!(text(&app), "aba", "failed read");
        } else {
            // A read that succeeds says what the clipboard holds now.
            assert_eq!(text(&app), "abold");
        }
    }
}

#[test]
fn the_register_stops_winning_once_the_system_clipboard_works_again() {
    let (mut app, clip) = clip_app("ab");
    clip.set_writes_fail(true);
    press(&mut app, shift(KeyCode::Right), 1);
    app.handle_key(ctrl('c'), t(1000));
    clip.set_writes_fail(false);
    // A later successful copy clears the flag: an empty clipboard is now "no text".
    app.handle_key(ctrl('c'), t(1000));
    clip.set_contents(None);
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab");
    assert_eq!(message(&app), "Clipboard has no text");

    // A later successful read clears it too.
    clip.set_writes_fail(true);
    press(&mut app, shift(KeyCode::Left), 1);
    app.handle_key(ctrl('c'), t(1000));
    app.handle_key(key(KeyCode::End), t(1000));
    clip.set_writes_fail(false);
    clip.set_contents(Some("sys"));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "absys");
    clip.set_contents(None);
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "absys", "the register is no longer newer");
}

#[test]
fn the_system_clipboard_wins_over_the_register() {
    let (mut app, clip) = clip_app("ab");
    press(&mut app, shift(KeyCode::Right), 1);
    app.handle_key(ctrl('c'), t(1000));
    clip.set_contents(Some("from elsewhere"));
    app.handle_key(key(KeyCode::End), t(1000));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "abfrom elsewhere");
}

#[test]
fn paste_with_nothing_anywhere_says_so_and_changes_nothing() {
    let (mut app, _clip) = clip_app("ab");
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab");
    assert!(!app.editor().buffer().is_modified());
    assert_eq!(message(&app), "Clipboard has no text");

    let (mut app, clip) = clip_app("ab");
    clip.set_broken(true);
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "ab");
    assert_eq!(message(&app), "Nothing to paste");
}

#[test]
fn clipboard_text_with_crlf_is_normalised_on_paste() {
    let (mut app, clip) = clip_app("");
    clip.set_contents(Some("a\r\nb\rc"));
    app.handle_key(ctrl('v'), t(1000));
    assert_eq!(text(&app), "a\nb\nc");
}

// ---- commands ----

#[test]
fn copy_cut_paste_and_selectall_commands_work() {
    let (mut app, clip) = clip_app("alpha beta");
    run(&mut app, "selectall");
    assert_eq!(app.editor().buffer().selected_text().as_deref(), Some("alpha beta"));
    run(&mut app, "copy");
    assert_eq!(clip.contents().as_deref(), Some("alpha beta"));
    assert_eq!(message(&app), "Copied 10 chars");
    run(&mut app, "cut");
    assert_eq!(text(&app), "");
    assert_eq!(message(&app), "Cut 10 chars");
    run(&mut app, "paste");
    assert_eq!(text(&app), "alpha beta");
    undo(&mut app);
    assert_eq!(text(&app), "");
}

#[test]
fn copy_and_cut_commands_without_a_selection_say_nothing_selected() {
    for command in ["copy", "cut"] {
        let (mut app, _clip) = clip_app("abc");
        run(&mut app, command);
        assert_eq!(message(&app), "Nothing selected", "{command}");
        assert_eq!(text(&app), "abc");
    }
}

#[test]
fn the_command_prompt_keeps_a_selection_that_was_there_when_it_opened() {
    let (mut app, clip) = clip_app("abc");
    press(&mut app, shift(KeyCode::Right), 2);
    app.handle_key(ctrl('x'), t(1000));
    type_str(&mut app, "copy");
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert_eq!(clip.contents(), None, "Esc ran nothing");
    assert_eq!(app.editor().buffer().selected_text().as_deref(), Some("ab"));
    run(&mut app, "copy");
    assert_eq!(clip.contents().as_deref(), Some("ab"));
}

// ---- App::paste (bracketed paste) ----

#[test]
fn bracketed_paste_inserts_multi_line_text_as_one_undo_step() {
    let (mut app, _clip) = clip_app("ab");
    app.handle_key(key(KeyCode::Right), t(1000));
    app.paste("1\n2\n3");
    assert_eq!(text(&app), "a1\n2\n3b");
    assert_eq!(app.editor().buffer().cursor(), (2, 1));
    assert!(app.editor().buffer().is_modified());
    undo(&mut app);
    assert_eq!(text(&app), "ab");
    assert!(!app.editor().buffer().is_modified());
}

#[test]
fn bracketed_paste_normalises_crlf_and_lone_cr() {
    let (mut app, _clip) = clip_app("");
    app.paste("a\r\nb\rc\r\n");
    assert_eq!(text(&app), "a\nb\nc\n");
    undo(&mut app);
    assert_eq!(text(&app), "");
}

#[test]
fn bracketed_paste_replaces_a_selection_in_one_step_and_keeps_tabs() {
    let (mut app, _clip) = clip_app("keep drop keep");
    app.handle_key(key(KeyCode::Right), t(1000));
    press(&mut app, shift(KeyCode::Right), 3);
    app.paste("\tX");
    assert_eq!(text(&app), "k\tX drop keep");
    undo(&mut app);
    assert_eq!(text(&app), "keep drop keep");
}

#[test]
fn bracketed_paste_does_not_touch_the_clipboard_or_the_register() {
    let (mut app, clip) = clip_app("x");
    clip.set_contents(Some("clip"));
    app.paste("pasted");
    assert_eq!(clip.contents().as_deref(), Some("clip"));
    app.handle_key(ctrl('v'), t(1000));
    assert!(text(&app).contains("clip"));
}

#[test]
fn an_empty_bracketed_paste_does_nothing() {
    let (mut app, _clip) = clip_app("abc");
    app.paste("");
    assert_eq!(text(&app), "abc");
    assert!(!app.editor().buffer().can_undo());
}

#[test]
fn a_five_megabyte_paste_is_one_fast_undo_step() {
    let (mut app, _clip) = clip_app("head\ntail");
    let chunk = "0123456789 abcdefghij klmnopqrst\r\n";
    let big = chunk.repeat(5 * 1024 * 1024 / chunk.len() + 1);
    assert!(big.len() > 5 * 1024 * 1024);
    app.handle_key(key(KeyCode::Down), t(1000));
    let start = std::time::Instant::now();
    app.paste(&big);
    let screen = app.screen(80, 24, t(1000));
    let took = start.elapsed();
    assert!(took < Duration::from_secs(2), "paste and first frame took {took:?}");
    assert_eq!(text(&app).len(), "head\ntail".len() + big.len() - big.matches('\r').count());
    assert!(!screen.rows.is_empty());
    let start = std::time::Instant::now();
    undo(&mut app);
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(text(&app), "head\ntail");
}

#[test]
fn a_five_megabyte_clipboard_paste_is_fast_too() {
    let (mut app, clip) = clip_app("");
    let big = "x".repeat(5 * 1024 * 1024);
    clip.set_contents(Some(&big));
    let start = std::time::Instant::now();
    app.handle_key(ctrl('v'), t(1000));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(text(&app).len(), big.len());
}

#[test]
fn bracketed_paste_clears_the_startup_hint_and_withdraws_warnings() {
    let (mut app, _clip) = clip_app("");
    assert_eq!(message(&app), "F1: help");
    app.paste("x");
    assert_eq!(message(&app), "");

    app.handle_key(ctrl('q'), t(1000));
    assert!(message(&app).contains("Unsaved"));
    app.paste("y");
    assert_eq!(message(&app), "");
    assert_eq!(app.handle_key(ctrl('q'), t(1000)), Flow::Continue, "the warning was withdrawn");
}

#[test]
fn hostile_pasted_text_is_drawn_sanitized_and_saved_byte_for_byte() {
    let dir = temp_dir("hostile-paste");
    let path = dir.join("p.txt");
    fs::write(&path, "").unwrap();
    let mut app = app_with(Editor::new(Buffer::open(&path).unwrap()), dir.clone(), FakeGit::new());
    let hostile = "a \x1b[31mred\x1b[2J \x1b]0;t\x07 \u{9b} \u{202e} \x7f\ttab";
    app.paste(hostile);
    let screen = app.screen(60, 6, t(1000));
    for row in &screen.rows {
        for span in row {
            assert!(!span.text.chars().any(char::is_control), "{:?}", span.text);
            assert!(!span.text.contains('\u{202e}'));
        }
    }
    assert!(row_text(&screen, 0).contains("a ?[31mred?[2J ?]0;t?"), "{}", row_text(&screen, 0));
    assert!(!screen.message.chars().any(char::is_control));
    app.handle_key(ctrl('s'), t(1000));
    assert_eq!(fs::read(&path).unwrap(), hostile.as_bytes());
}

// ---- paste into prompts ----

#[test]
fn paste_into_find_removes_line_breaks_and_searches() {
    let (mut app, _clip) = clip_app("zero\nline 3 here\nlast");
    app.handle_key(ctrl('f'), t(1000));
    app.paste("line\n 3\r\n");
    assert!(app.editor().in_prompt());
    let message = app.screen(60, 6, t(1000)).message;
    assert_eq!(message, "Find: line 3_");
    assert_eq!(app.editor().buffer().cursor(), (1, 0));
    assert_eq!(text(&app), "zero\nline 3 here\nlast");
    app.handle_key(key(KeyCode::Esc), t(1000));
    assert_eq!(app.editor().buffer().cursor(), (0, 0));
}

#[test]
fn paste_into_find_after_typing_appends() {
    let (mut app, _clip) = clip_app("abc abd");
    app.handle_key(ctrl('f'), t(1000));
    type_str(&mut app, "a");
    app.paste("bd");
    assert_eq!(app.screen(40, 6, t(1000)).message, "Find: abd_");
    assert_eq!(app.editor().buffer().cursor(), (0, 4));
}

#[test]
fn paste_into_goto_keeps_digits_only() {
    let (mut app, _clip) = clip_app("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12");
    app.handle_key(ctrl('l'), t(1000));
    app.paste("1a\n2 x\r\n");
    assert_eq!(app.screen(40, 6, t(1000)).message, "Go to line: 12_");
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert_eq!(app.editor().buffer().cursor(), (11, 0));
    assert_eq!(text(&app), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12");
}

#[test]
fn paste_with_a_newline_into_the_command_prompt_runs_nothing() {
    let (mut app, _clip) = clip_app("abc");
    type_str(&mut app, "x");
    assert!(app.editor().buffer().is_modified());
    app.handle_key(ctrl('x'), t(1000));
    app.paste("quit\n");
    assert!(app.editor().in_prompt(), "the prompt is still open");
    assert_eq!(app.screen(60, 6, t(1000)).message.split("  ").next().unwrap(), "Command: quit_");
    app.paste("undo\r\nredo\r");
    assert!(app.editor().in_prompt());
    assert_eq!(text(&app), "xabc", "no command ran");
    assert!(app.editor().buffer().is_modified());
    // Enter is still what runs it (here: an unknown command).
    app.handle_key(key(KeyCode::Enter), t(1000));
    assert!(!app.editor().in_prompt());
    assert!(message(&app).starts_with("Unknown command: quitundoredo"), "{}", message(&app));
}

#[test]
fn prompt_paste_is_capped_and_does_not_edit_the_buffer() {
    let (mut app, _clip) = clip_app("abc");
    app.handle_key(ctrl('x'), t(1000));
    app.paste(&"z".repeat(1 << 20));
    assert!(app.editor().in_prompt());
    assert_eq!(text(&app), "abc");
    assert!(!app.editor().buffer().is_modified());
    let screen = app.screen(40, 6, t(1000));
    assert!(screen.message.chars().count() <= 40);
}

// ---- views and help ----

#[test]
fn bracketed_paste_is_ignored_in_the_views_and_help() {
    for enter in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        let (mut app, _clip) = clip_app("abc");
        app.handle_key(enter, t(1000));
        let before = app.screen(40, 8, t(1000));
        app.paste("junk\nmore");
        assert_eq!(app.screen(40, 8, t(1000)), before);
        assert_eq!(text(&app), "abc");
        assert!(!app.editor().buffer().is_modified());
        assert!(!app.editor().buffer().can_undo());
    }
}

#[test]
fn ctrl_c_k_v_do_nothing_in_the_views_and_help() {
    for enter in [ctrl('g'), ctrl('t'), ctrl('o'), key(KeyCode::F(1))] {
        let (mut app, clip) = clip_app("abc");
        app.handle_key(ctrl('a'), t(1000));
        clip.set_contents(Some("clip"));
        app.handle_key(enter, t(1000));
        for combo in [ctrl('c'), ctrl('k'), ctrl('v')] {
            app.handle_key(combo, t(1000));
        }
        assert_eq!(text(&app), "abc");
        assert_eq!(clip.contents().as_deref(), Some("clip"));
        assert!(!app.editor().buffer().can_undo());
    }
}

#[test]
fn ctrl_c_k_v_are_ignored_in_prompts() {
    for open in ['f', 'l', 'x'] {
        let (mut app, clip) = clip_app("abc def");
        press(&mut app, shift(KeyCode::Right), 3);
        clip.set_contents(Some("clip"));
        app.handle_key(ctrl(open), t(1000));
        let before = app.screen(60, 6, t(1000)).message;
        for combo in [ctrl('c'), ctrl('k'), ctrl('v')] {
            app.handle_key(combo, t(1000));
        }
        assert!(app.editor().in_prompt(), "prompt {open}");
        assert_eq!(app.screen(60, 6, t(1000)).message, before, "prompt {open}");
        assert_eq!(text(&app), "abc def");
        assert!(!app.editor().buffer().can_undo());
        assert_eq!(clip.contents().as_deref(), Some("clip"));
    }
}

// ---- the selection is cleared when buffers change (review follow-up) ----

fn two_files(name: &str) -> (App, Temp) {
    let dir = Temp::new(name);
    dir.file("a.txt", "alpha\nsecond\n");
    dir.file("b.txt", "beta\n");
    let buffer = Buffer::open(&dir.0.join("a.txt")).unwrap();
    let app = app_with(Editor::new(buffer), dir.0.clone(), one_file());
    (app, dir)
}

#[test]
fn opening_onto_an_existing_buffer_clears_its_selection() {
    // Edited (kept as is), unedited and unchanged on disk (kept as is), and
    // unedited but changed on disk (replaced): none keeps a selection.
    for mode in ["edited", "unchanged", "changed"] {
        let (mut app, dir) = two_files("open-clears");
        open_from_tree(&mut app, 1); // b.txt is current, a.txt is index 0
        if mode == "edited" {
            app.handle_key(ctrl('p'), t(1000));
            type_str(&mut app, "X");
            app.handle_key(ctrl('n'), t(1000));
        }
        app.editors[0].handle_key(ctrl('a'));
        assert!(app.editors[0].buffer().selection().is_some());
        if mode == "changed" {
            dir.file("a.txt", "changed\n");
        }
        open_from_tree(&mut app, 0);
        assert_eq!(app.current_index(), 0, "{mode}");
        assert!(app.editors[0].buffer().selection().is_none(), "{mode}");
        assert!(app.editors[1].buffer().selection().is_none(), "{mode}");
    }
}

#[test]
fn opening_a_new_file_clears_the_selection_of_the_buffer_left() {
    let (mut app, _dir) = two_files("add-clears");
    app.handle_key(ctrl('a'), t(1000));
    assert!(app.editor().buffer().selection().is_some());
    open_from_tree(&mut app, 1);
    assert_eq!(app.buffer_count(), 2);
    assert_eq!(app.current_index(), 1);
    assert!(app.editors[0].buffer().selection().is_none());
    assert!(app.editors[1].buffer().selection().is_none());
}

#[test]
fn the_command_prompt_next_prev_and_close_clear_the_selection() {
    let (mut app, _dir) = two_files("command-clears");
    open_from_tree(&mut app, 1);
    for command in ["next", "prev"] {
        app.editors[0].handle_key(ctrl('a'));
        app.editors[1].handle_key(ctrl('a'));
        run(&mut app, command);
        assert!(app.editors.iter().all(|e| e.buffer().selection().is_none()), "{command}");
    }
    // close: the buffer that becomes current shows no selection.
    app.editors[0].handle_key(ctrl('a'));
    app.editors[1].handle_key(ctrl('a'));
    run(&mut app, "close");
    assert_eq!(app.buffer_count(), 1);
    assert!(app.editors[0].buffer().selection().is_none());
}
