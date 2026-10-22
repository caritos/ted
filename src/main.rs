use std::io::{self, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};

use ted::app::App;
use ted::buffer::Buffer;
use ted::editor::{Editor, Flow};
use ted::git::GitCli;
use ted::screen::{Screen, Style};

/// How often the git view polls git.
const TICK: Duration = Duration::from_millis(1000);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (buffer, location) = match args.as_slice() {
        [] => (Buffer::new(), PathBuf::from(".")),
        [path] if Path::new(path).is_dir() => (Buffer::new(), PathBuf::from(path)),
        [path] => match Buffer::open(Path::new(path)) {
            Ok(buffer) => (buffer, parent_of(Path::new(path))),
            Err(e) => {
                eprintln!("ted: {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: ted [file|dir]");
            return ExitCode::from(2);
        }
    };

    let source = GitCli::new(location.clone());
    let app = App::new(Editor::new(buffer), location, Box::new(source));
    match run(app) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ted: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The directory containing `path`, or `.` when it has none.
fn parent_of(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn run(mut app: App) -> io::Result<()> {
    let _terminal = TerminalGuard::enter()?;
    let mut out = io::stdout();
    let mut drawn: Option<(Screen, (u16, u16))> = None;
    // A fixed deadline rather than a fresh timeout per event, so a stream
    // of keys (a held arrow) cannot hold off the refresh.
    let mut next_tick = Instant::now() + TICK;
    loop {
        let size = terminal::size()?;
        let screen = app.screen(size.0 as usize, size.1 as usize, SystemTime::now());
        if drawn.as_ref().is_none_or(|(s, sz)| *s != screen || *sz != size) {
            draw(&mut out, &screen, size.1)?;
            drawn = Some((screen, size));
        }

        let now = Instant::now();
        if now >= next_tick {
            app.tick(SystemTime::now());
            next_tick = now + TICK;
            continue;
        }
        // Anything other than a key press (including a resize) just redraws.
        if event::poll(next_tick - now)?
            && let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
            && app.handle_key(key, SystemTime::now()) == Flow::Quit
        {
            return Ok(());
        }
    }
}

fn draw(out: &mut Stdout, screen: &Screen, height: u16) -> io::Result<()> {
    queue!(out, Hide)?;
    let mut y: u16 = 0;
    for row in &screen.rows {
        if y >= height {
            break;
        }
        queue!(out, MoveTo(0, y))?;
        for span in row {
            set_style(out, span.style)?;
            queue!(out, Print(&span.text))?;
            queue!(out, SetAttribute(Attribute::Reset), ResetColor)?;
        }
        queue!(out, Clear(ClearType::UntilNewLine))?;
        y += 1;
    }
    if y < height {
        queue!(
            out,
            MoveTo(0, y),
            SetAttribute(Attribute::Reverse),
            Print(&screen.status),
            SetAttribute(Attribute::NoReverse),
            Clear(ClearType::UntilNewLine)
        )?;
        y += 1;
    }
    if y < height {
        queue!(out, MoveTo(0, y), Print(&screen.message), Clear(ClearType::UntilNewLine))?;
    }
    if let Some((row, col)) = screen.cursor
        && let (Ok(row), Ok(col)) = (u16::try_from(row), u16::try_from(col))
    {
        queue!(out, MoveTo(col, row), Show)?;
    }
    out.flush()
}

fn set_style(out: &mut Stdout, style: Style) -> io::Result<()> {
    match style {
        Style::Normal => Ok(()),
        Style::Selected => queue!(out, SetAttribute(Attribute::Reverse)),
        Style::Added => queue!(out, SetForegroundColor(Color::Green)),
        Style::Removed => queue!(out, SetForegroundColor(Color::Red)),
        Style::Hunk => queue!(out, SetForegroundColor(Color::Cyan)),
        Style::Dim => queue!(out, SetAttribute(Attribute::Dim)),
    }
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
