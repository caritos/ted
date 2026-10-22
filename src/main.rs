use std::io::{self, Stdout, Write};
use std::path::Path;
use std::process::ExitCode;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};

use ted::buffer::Buffer;
use ted::editor::{Editor, Flow, View};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let buffer = match args.as_slice() {
        [] => Buffer::new(),
        [path] => match Buffer::open(Path::new(path)) {
            Ok(buffer) => buffer,
            Err(e) => {
                eprintln!("ted: {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: ted [file]");
            return ExitCode::from(2);
        }
    };

    match run(Editor::new(buffer)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ted: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut editor: Editor) -> io::Result<()> {
    let _terminal = TerminalGuard::enter()?;
    let mut out = io::stdout();
    loop {
        let (width, height) = terminal::size()?;
        let view = editor.view(width as usize, height as usize);
        draw(&mut out, &view, height)?;

        // Anything other than a key press (including a resize) just redraws.
        if let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
            && editor.handle_key(key) == Flow::Quit
        {
            return Ok(());
        }
    }
}

fn draw(out: &mut Stdout, view: &View, height: u16) -> io::Result<()> {
    queue!(out, Hide)?;
    let mut y: u16 = 0;
    for line in &view.lines {
        queue!(out, MoveTo(0, y), Print(line), Clear(ClearType::UntilNewLine))?;
        y += 1;
    }
    if y < height {
        queue!(
            out,
            MoveTo(0, y),
            SetAttribute(Attribute::Reverse),
            Print(&view.status),
            SetAttribute(Attribute::NoReverse)
        )?;
        y += 1;
    }
    if y < height {
        queue!(out, MoveTo(0, y), Print(&view.message), Clear(ClearType::UntilNewLine))?;
    }
    let (row, col) = view.cursor;
    queue!(out, MoveTo(col as u16, row as u16), Show)?;
    out.flush()
}

/// Puts the terminal in raw mode on the alternate screen and restores it on
/// drop, including when unwinding from a panic.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, Clear(ClearType::All)) {
            let _ = disable_raw_mode();
            return Err(e);
        }
        // Restore the terminal before the panic message prints, or it would
        // be swallowed by the alternate screen.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn restore() {
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}
