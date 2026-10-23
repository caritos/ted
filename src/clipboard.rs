//! The clipboard behind a trait: the system one (command line tools, best
//! effort, never blocks the UI for long) and an in-memory one for tests.
//! No terminal code.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub trait Clipboard {
    /// The clipboard text, or `None` when it is unavailable or not text.
    fn get(&mut self) -> Option<String>;
    /// Replaces the clipboard text; false when that was not possible.
    fn set(&mut self, text: &str) -> bool;
}

/// How long one `get` or `set` may take in total.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// What running one tool came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Exited with status 0; its stdout (empty for a copy).
    Done(Vec<u8>),
    /// Could not be started (not installed).
    Missing,
    /// Started but exited non-zero or its output could not be read.
    Failed,
    /// Still running at the deadline; it was killed.
    TimedOut,
}

/// Runs one command line; the seam `SystemClipboard` is tested through.
pub trait Runner {
    /// Runs `argv` with `input` on stdin (none: stdin is empty) and returns
    /// its stdout, giving up after `timeout`. Stdout is only collected when
    /// `want_output` is set.
    fn run(&mut self, argv: &[&str], input: Option<&[u8]>, want_output: bool, timeout: Duration) -> Outcome;
}

/// The real runner: `std::process::Command` with a deadline.
pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&mut self, argv: &[&str], input: Option<&[u8]>, want_output: bool, timeout: Duration) -> Outcome {
        let Some((program, args)) = argv.split_first() else {
            return Outcome::Missing;
        };
        let deadline = Instant::now() + timeout;
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(if want_output { Stdio::piped() } else { Stdio::null() })
            .stderr(Stdio::null());
        let Ok(mut child) = command.spawn() else {
            return Outcome::Missing;
        };
        // Threads, so a tool that does not read its input or does not close
        // its output cannot stall the loop below.
        if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
            let data = data.to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&data);
            });
        }
        let (tx, rx) = mpsc::channel();
        if let Some(mut stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = tx.send(stdout.read_to_end(&mut bytes).map(|_| bytes));
            });
        }
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Outcome::TimedOut;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => return Outcome::Failed,
            }
        };
        if !status.success() {
            return Outcome::Failed;
        }
        if !want_output {
            return Outcome::Done(Vec::new());
        }
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(bytes)) => Outcome::Done(bytes),
            Ok(Err(_)) => Outcome::Failed,
            Err(_) => Outcome::TimedOut,
        }
    }
}

/// (paste command, copy command) for each supported tool, in the order tried.
const TOOLS: [(&[&str], &[&str]); 3] = [
    (&["pbpaste"], &["pbcopy"]),
    (&["wl-paste", "--no-newline"], &["wl-copy"]),
    (&["xclip", "-selection", "clipboard", "-o"], &["xclip", "-selection", "clipboard", "-i"]),
];

/// The system clipboard through `pbcopy`/`pbpaste`, `wl-copy`/`wl-paste` or
/// `xclip`, whichever is installed. A missing tool, a non-zero exit, output
/// that is not UTF-8 and a timeout all mean "unavailable". One call takes at
/// most `TIMEOUT` however many tools it tries.
pub struct SystemClipboard<R: Runner = ProcessRunner> {
    runner: R,
}

impl SystemClipboard {
    pub fn new() -> Self {
        SystemClipboard { runner: ProcessRunner }
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runner> SystemClipboard<R> {
    pub fn with_runner(runner: R) -> Self {
        SystemClipboard { runner }
    }
}

impl<R: Runner> Clipboard for SystemClipboard<R> {
    fn get(&mut self) -> Option<String> {
        let deadline = Instant::now() + TIMEOUT;
        for (paste, _) in TOOLS {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            match self.runner.run(paste, None, true, left) {
                Outcome::Done(bytes) => {
                    if let Ok(text) = String::from_utf8(bytes) {
                        return Some(text);
                    }
                }
                Outcome::Missing | Outcome::Failed => {}
                Outcome::TimedOut => return None,
            }
        }
        None
    }

    fn set(&mut self, text: &str) -> bool {
        let deadline = Instant::now() + TIMEOUT;
        for (_, copy) in TOOLS {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            match self.runner.run(copy, Some(text.as_bytes()), false, left) {
                Outcome::Done(_) => return true,
                Outcome::Missing | Outcome::Failed => {}
                Outcome::TimedOut => return false,
            }
        }
        false
    }
}

#[derive(Debug, Default)]
struct MemoryState {
    text: Option<String>,
    broken: bool,
}

/// A clipboard in memory. Clones share the text, so a test can hand one to
/// the app and keep another to look at.
#[derive(Debug, Clone, Default)]
pub struct MemoryClipboard {
    state: Rc<RefCell<MemoryState>>,
}

impl MemoryClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// The text held, whatever `set_broken` says.
    pub fn contents(&self) -> Option<String> {
        self.state.borrow().text.clone()
    }

    pub fn set_contents(&self, text: Option<&str>) {
        self.state.borrow_mut().text = text.map(str::to_string);
    }

    /// A broken clipboard fails every `get` and `set`, like a missing tool.
    pub fn set_broken(&self, broken: bool) {
        self.state.borrow_mut().broken = broken;
    }
}

impl Clipboard for MemoryClipboard {
    fn get(&mut self) -> Option<String> {
        let state = self.state.borrow();
        if state.broken { None } else { state.text.clone() }
    }

    fn set(&mut self, text: &str) -> bool {
        let mut state = self.state.borrow_mut();
        if state.broken {
            return false;
        }
        state.text = Some(text.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers by program name, records every call.
    struct Fake {
        answers: Vec<(&'static str, Outcome)>,
        calls: Vec<(String, Option<Vec<u8>>, bool, Duration)>,
    }

    impl Fake {
        fn new(answers: Vec<(&'static str, Outcome)>) -> Self {
            Fake { answers, calls: Vec::new() }
        }
    }

    impl Runner for Fake {
        fn run(&mut self, argv: &[&str], input: Option<&[u8]>, want_output: bool, timeout: Duration) -> Outcome {
            self.calls.push((argv[0].to_string(), input.map(<[u8]>::to_vec), want_output, timeout));
            match self.answers.iter().position(|(name, _)| *name == argv[0]) {
                Some(i) => std::mem::replace(&mut self.answers[i].1, Outcome::Missing),
                None => Outcome::Missing,
            }
        }
    }

    fn names(fake: &Fake) -> Vec<&str> {
        fake.calls.iter().map(|c| c.0.as_str()).collect()
    }

    #[test]
    fn get_reads_utf8_from_the_first_tool_that_works() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![("pbpaste", Outcome::Done("héllo\n".into()))]));
        assert_eq!(clip.get().as_deref(), Some("héllo\n"));
        assert_eq!(names(&clip.runner), vec!["pbpaste"]);
        assert!(clip.runner.calls[0].2, "output is wanted");
    }

    #[test]
    fn get_keeps_an_empty_clipboard_as_empty_text() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![("pbpaste", Outcome::Done(Vec::new()))]));
        assert_eq!(clip.get().as_deref(), Some(""));
    }

    #[test]
    fn get_with_no_tool_installed_is_unavailable() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![]));
        assert_eq!(clip.get(), None);
        assert_eq!(names(&clip.runner), vec!["pbpaste", "wl-paste", "xclip"]);
    }

    #[test]
    fn get_falls_through_missing_and_failing_tools() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![
            ("pbpaste", Outcome::Missing),
            ("wl-paste", Outcome::Failed),
            ("xclip", Outcome::Done(b"from xclip".to_vec())),
        ]));
        assert_eq!(clip.get().as_deref(), Some("from xclip"));
    }

    #[test]
    fn get_treats_non_utf8_output_as_unavailable() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![("pbpaste", Outcome::Done(vec![0x66, 0xff, 0xfe]))]));
        assert_eq!(clip.get(), None);
    }

    #[test]
    fn get_gives_up_at_the_first_timeout() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![
            ("pbpaste", Outcome::TimedOut),
            ("wl-paste", Outcome::Done(b"late".to_vec())),
        ]));
        assert_eq!(clip.get(), None);
        assert_eq!(names(&clip.runner), vec!["pbpaste"]);
    }

    #[test]
    fn every_call_is_bounded_by_the_timeout() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![]));
        clip.get();
        clip.set("x");
        assert!(clip.runner.calls.iter().all(|c| c.3 <= TIMEOUT && !c.3.is_zero()));
        assert!(TIMEOUT <= Duration::from_secs(2));
    }

    #[test]
    fn set_sends_the_text_on_stdin_and_expects_no_output() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![("pbcopy", Outcome::Done(Vec::new()))]));
        assert!(clip.set("a\nb"));
        let call = &clip.runner.calls[0];
        assert_eq!(call.0, "pbcopy");
        assert_eq!(call.1.as_deref(), Some(&b"a\nb"[..]));
        assert!(!call.2);
    }

    #[test]
    fn set_fails_when_no_tool_works_and_stops_at_a_timeout() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![("pbcopy", Outcome::Failed)]));
        assert!(!clip.set("x"));
        assert_eq!(names(&clip.runner), vec!["pbcopy", "wl-copy", "xclip"]);

        let mut clip = SystemClipboard::with_runner(Fake::new(vec![
            ("pbcopy", Outcome::TimedOut),
            ("wl-copy", Outcome::Done(Vec::new())),
        ]));
        assert!(!clip.set("x"));
        assert_eq!(names(&clip.runner), vec!["pbcopy"]);
    }

    #[test]
    fn set_falls_through_to_the_next_tool() {
        let mut clip = SystemClipboard::with_runner(Fake::new(vec![
            ("pbcopy", Outcome::Missing),
            ("wl-copy", Outcome::Done(Vec::new())),
        ]));
        assert!(clip.set("x"));
        assert_eq!(names(&clip.runner), vec!["pbcopy", "wl-copy"]);
    }

    #[test]
    fn memory_clipboard_clones_share_text_and_can_break() {
        let a = MemoryClipboard::new();
        let mut b = a.clone();
        assert_eq!(b.get(), None);
        assert!(b.set("hi"));
        assert_eq!(a.contents().as_deref(), Some("hi"));
        a.set_broken(true);
        assert_eq!(b.get(), None);
        assert!(!b.set("lost"));
        assert_eq!(a.contents().as_deref(), Some("hi"));
    }

    // The real runner, with harmless programs; never the clipboard tools.

    #[cfg(unix)]
    mod process {
        use super::*;

        fn run(argv: &[&str], input: Option<&[u8]>, want: bool, ms: u64) -> Outcome {
            ProcessRunner.run(argv, input, want, Duration::from_millis(ms))
        }

        #[test]
        fn collects_stdout_and_feeds_stdin() {
            assert_eq!(run(&["printf", "hi"], None, true, 2000), Outcome::Done(b"hi".to_vec()));
            assert_eq!(run(&["cat"], Some(b"round trip"), true, 2000), Outcome::Done(b"round trip".to_vec()));
            assert_eq!(run(&["cat"], Some(b"ignored"), false, 2000), Outcome::Done(Vec::new()));
        }

        #[test]
        fn a_missing_command_is_missing() {
            assert_eq!(run(&["ted-no-such-tool-xyz"], None, true, 2000), Outcome::Missing);
            assert_eq!(run(&[], None, true, 2000), Outcome::Missing);
        }

        #[test]
        fn a_non_zero_exit_is_failed() {
            assert_eq!(run(&["sh", "-c", "echo out; exit 3"], None, true, 2000), Outcome::Failed);
        }

        #[test]
        fn a_slow_command_times_out_and_is_killed_promptly() {
            let start = Instant::now();
            assert_eq!(run(&["sleep", "10"], None, true, 150), Outcome::TimedOut);
            assert!(start.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn a_tool_that_never_reads_its_input_times_out_without_hanging() {
            let big = vec![b'x'; 8 << 20];
            let start = Instant::now();
            assert_eq!(run(&["sleep", "10"], Some(&big), false, 150), Outcome::TimedOut);
            assert!(start.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn a_background_child_holding_stdout_open_times_out_instead_of_hanging() {
            let start = Instant::now();
            let outcome = run(&["sh", "-c", "sleep 5 & echo hi"], None, true, 200);
            assert_eq!(outcome, Outcome::TimedOut);
            assert!(start.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn non_utf8_output_comes_back_as_bytes_for_the_caller_to_judge() {
            assert_eq!(run(&["printf", "\\377"], None, true, 2000), Outcome::Done(vec![0xff]));
        }
    }
}
