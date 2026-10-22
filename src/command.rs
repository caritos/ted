//! Command prompt: names, prefix matching, completion and parsing.
//! Pure logic, no terminal and no editor state.

/// Canonical command names, in the order they are listed and completed.
pub const NAMES: [&str; 11] = [
    "save", "goto", "lines", "next", "prev", "close", "buffers", "tree", "git", "help", "quit",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Save,
    Goto(usize),
    Lines(Option<bool>),
    Next,
    Prev,
    Close,
    Buffers,
    Tree,
    Git,
    Help,
    Quit,
    WriteQuit,
}

/// Names that take an argument; Tab adds a space after completing one.
const TAKES_ARGUMENT: [&str; 2] = ["goto", "lines"];

/// Names starting with the (case-insensitive) word, in `NAMES` order.
pub fn matches(word: &str) -> Vec<&'static str> {
    let word = word.to_lowercase();
    NAMES.iter().copied().filter(|n| n.starts_with(&word)).collect()
}

/// Tab result for the command word: the longest common prefix of the
/// matching names, plus a space when that is one full name taking an
/// argument. The word itself when nothing matches.
pub fn complete(word: &str) -> String {
    let found = matches(word);
    let Some((first, rest)) = found.split_first() else {
        return word.to_string();
    };
    let mut prefix = first.len();
    for name in rest {
        let common = first.chars().zip(name.chars()).take_while(|(a, b)| a == b).count();
        prefix = prefix.min(common);
    }
    let mut result = first[..prefix].to_string();
    if TAKES_ARGUMENT.contains(&result.as_str()) {
        result.push(' ');
    }
    result
}

/// Parses what was typed into a command. `Err` carries the message to show;
/// it is empty when there is nothing to run.
pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim();
    let mut words = input.split_whitespace();
    let Some(first) = words.next() else {
        return Err(String::new());
    };
    let args: Vec<&str> = words.collect();
    let word = first.to_lowercase();
    let name = match word.as_str() {
        "w" => "save",
        "q" => "quit",
        "wq" => "wq",
        _ if NAMES.contains(&word.as_str()) => word.as_str(),
        _ => {
            let found = matches(&word);
            match found.as_slice() {
                [] => return Err(format!("Unknown command: {first}")),
                [one] => *one,
                _ => return Err(format!("Ambiguous: {}", found.join(", "))),
            }
        }
    };
    let plain = |command: Command| {
        if args.is_empty() {
            Ok(command)
        } else {
            Err(format!("Usage: {name}"))
        }
    };
    match name {
        "save" => plain(Command::Save),
        "next" => plain(Command::Next),
        "prev" => plain(Command::Prev),
        "close" => plain(Command::Close),
        "buffers" => plain(Command::Buffers),
        "tree" => plain(Command::Tree),
        "git" => plain(Command::Git),
        "help" => plain(Command::Help),
        "quit" => plain(Command::Quit),
        "wq" => plain(Command::WriteQuit),
        "goto" => match args.as_slice() {
            [n] => match n.parse::<usize>() {
                Ok(n) if n > 0 => Ok(Command::Goto(n)),
                _ => Err("Usage: goto <line>".to_string()),
            },
            _ => Err("Usage: goto <line>".to_string()),
        },
        "lines" => match args.as_slice() {
            [] => Ok(Command::Lines(None)),
            ["on"] => Ok(Command::Lines(Some(true))),
            ["off"] => Ok(Command::Lines(Some(false))),
            _ => Err("Usage: lines [on|off]".to_string()),
        },
        _ => Err(format!("Unknown command: {first}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(input: &str) -> String {
        parse(input).unwrap_err()
    }

    #[test]
    fn parses_each_command() {
        assert_eq!(parse("save"), Ok(Command::Save));
        assert_eq!(parse("next"), Ok(Command::Next));
        assert_eq!(parse("prev"), Ok(Command::Prev));
        assert_eq!(parse("close"), Ok(Command::Close));
        assert_eq!(parse("buffers"), Ok(Command::Buffers));
        assert_eq!(parse("tree"), Ok(Command::Tree));
        assert_eq!(parse("git"), Ok(Command::Git));
        assert_eq!(parse("help"), Ok(Command::Help));
        assert_eq!(parse("quit"), Ok(Command::Quit));
    }

    #[test]
    fn aliases() {
        assert_eq!(parse("w"), Ok(Command::Save));
        assert_eq!(parse("q"), Ok(Command::Quit));
        assert_eq!(parse("wq"), Ok(Command::WriteQuit));
    }

    #[test]
    fn trims_and_ignores_case_in_the_name() {
        assert_eq!(parse("  SAVE  "), Ok(Command::Save));
        assert_eq!(parse("Goto 12"), Ok(Command::Goto(12)));
    }

    #[test]
    fn goto_needs_a_positive_number() {
        assert_eq!(parse("goto 12"), Ok(Command::Goto(12)));
        assert_eq!(parse("goto   7 "), Ok(Command::Goto(7)));
        for bad in ["goto", "goto 0", "goto x", "goto 1 2", "goto -3"] {
            assert_eq!(err(bad), "Usage: goto <line>", "{bad}");
        }
    }

    #[test]
    fn lines_takes_on_off_or_nothing() {
        assert_eq!(parse("lines"), Ok(Command::Lines(None)));
        assert_eq!(parse("lines on"), Ok(Command::Lines(Some(true))));
        assert_eq!(parse("lines off"), Ok(Command::Lines(Some(false))));
        assert_eq!(err("lines maybe"), "Usage: lines [on|off]");
        assert_eq!(err("lines on off"), "Usage: lines [on|off]");
    }

    #[test]
    fn unique_prefix_runs() {
        assert_eq!(parse("buf"), Ok(Command::Buffers));
        assert_eq!(parse("s"), Ok(Command::Save));
        assert_eq!(parse("qu"), Ok(Command::Quit));
    }

    #[test]
    fn ambiguous_prefix_lists_names_in_order() {
        assert_eq!(err("g"), "Ambiguous: goto, git");
    }

    #[test]
    fn extra_arguments_to_plain_commands_are_an_error() {
        assert_eq!(err("next now"), "Usage: next");
        assert_eq!(err("quit please"), "Usage: quit");
        assert_eq!(err("w now"), "Usage: save");
        assert_eq!(err("wq now"), "Usage: wq");
    }

    #[test]
    fn unknown_word() {
        assert_eq!(err("foo"), "Unknown command: foo");
        assert_eq!(err("foo bar"), "Unknown command: foo");
    }

    #[test]
    fn empty_input_is_silent() {
        assert_eq!(err(""), "");
        assert_eq!(err("   "), "");
    }

    #[test]
    fn matches_lists_prefixes_in_order() {
        assert_eq!(matches("g"), vec!["goto", "git"]);
        assert_eq!(matches("GO"), vec!["goto"]);
        assert_eq!(matches("zzz"), Vec::<&str>::new());
        assert_eq!(matches(""), NAMES.to_vec());
    }

    #[test]
    fn complete_cases() {
        assert_eq!(complete("go"), "goto ");
        assert_eq!(complete("g"), "g");
        assert_eq!(complete("qu"), "quit");
        assert_eq!(complete("zzz"), "zzz");
        assert_eq!(complete("LI"), "lines ");
        assert_eq!(complete("ZZZ"), "ZZZ");
        assert_eq!(complete("goto"), "goto ");
        assert_eq!(complete("bu"), "buffers");
    }
}
