//! Command prompt: names, prefix matching, completion and parsing.
//! Pure logic, no terminal and no editor state.

/// Canonical command names, in the order they are listed and completed.
pub const NAMES: [&str; 20] = [
    "save", "saveas", "goto", "lines", "next", "prev", "close", "buffers", "tree", "git", "help", "undo", "redo", "replace", "qreplace",
    "copy", "cut", "paste", "selectall", "quit",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Save,
    SaveAs(String),
    Goto(usize),
    Lines(Option<bool>),
    Next,
    Prev,
    Close,
    Buffers,
    Tree,
    Git,
    Help,
    Undo,
    Redo,
    Replace,
    /// Replace with a question for each match.
    ReplaceAsk,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Quit,
    WriteQuit,
}

/// Names that take an argument; Tab adds a space after completing one.
const TAKES_ARGUMENT: [&str; 3] = ["saveas", "goto", "lines"];

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
        "undo" => plain(Command::Undo),
        "redo" => plain(Command::Redo),
        "replace" => plain(Command::Replace),
        "qreplace" => plain(Command::ReplaceAsk),
        "copy" => plain(Command::Copy),
        "cut" => plain(Command::Cut),
        "paste" => plain(Command::Paste),
        "selectall" => plain(Command::SelectAll),
        "quit" => plain(Command::Quit),
        "wq" => plain(Command::WriteQuit),
        // The path keeps its case and inner spaces.
        "saveas" => match input[first.len()..].trim() {
            "" => Err("Usage: saveas <path>".to_string()),
            path => Ok(Command::SaveAs(path.to_string())),
        },
        "goto" => match args.as_slice() {
            // All digits: a number too big for usize clamps (to the last line).
            [n] if n.chars().all(|c| c.is_ascii_digit()) => match n.parse::<usize>() {
                Ok(0) => Err("Usage: goto <line>".to_string()),
                Ok(n) => Ok(Command::Goto(n)),
                Err(_) => Ok(Command::Goto(usize::MAX)),
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
        assert_eq!(parse("sav"), Err("Ambiguous: save, saveas".to_string()));
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

    #[test]
    fn saveas_parses_with_its_path() {
        assert_eq!(parse("saveas out.txt"), Ok(Command::SaveAs("out.txt".to_string())));
        assert_eq!(parse("SaveAs  My File.TXT "), Ok(Command::SaveAs("My File.TXT".to_string())));
        assert_eq!(err("saveas"), "Usage: saveas <path>");
        assert_eq!(err("saveas   "), "Usage: saveas <path>");
    }

    #[test]
    fn save_stays_exact_and_prefixes_of_both_are_ambiguous() {
        assert_eq!(parse("save"), Ok(Command::Save));
        assert_eq!(err("sa"), "Ambiguous: save, saveas");
        // `selectall` joined the names starting with s.
        assert_eq!(err("s"), "Ambiguous: save, saveas, selectall");
        assert_eq!(parse("savea x"), Ok(Command::SaveAs("x".to_string())));
        assert_eq!(matches("sa"), vec!["save", "saveas"]);
    }

    #[test]
    fn goto_overflow_clamps_but_other_bad_numbers_are_usage_errors() {
        assert_eq!(parse("goto 99999999999999999999999999"), Ok(Command::Goto(usize::MAX)));
        assert_eq!(err("goto -1"), "Usage: goto <line>");
        assert_eq!(err("goto 0"), "Usage: goto <line>");
        assert_eq!(err("goto 000"), "Usage: goto <line>");
        assert_eq!(err("goto 1x"), "Usage: goto <line>");
        assert_eq!(err("goto +5"), "Usage: goto <line>");
    }

    #[test]
    fn undo_and_redo_parse_without_arguments() {
        assert_eq!(parse("undo"), Ok(Command::Undo));
        assert_eq!(parse("redo"), Ok(Command::Redo));
        assert_eq!(parse("u"), Ok(Command::Undo));
        // `re` and `r` used to run redo; `replace` made them ambiguous.
        assert_eq!(err("re"), "Ambiguous: redo, replace");
        assert_eq!(err("r"), "Ambiguous: redo, replace");
        assert_eq!(parse("red"), Ok(Command::Redo));
        assert_eq!(err("undo 2"), "Usage: undo");
        assert_eq!(err("redo now"), "Usage: redo");
        assert_eq!(matches("u"), vec!["undo"]);
        assert_eq!(complete("un"), "undo");
        assert_eq!(complete("r"), "re");
        assert_eq!(complete("red"), "redo");
    }

    #[test]
    fn replace_parses_without_arguments() {
        assert_eq!(parse("replace"), Ok(Command::Replace));
        assert_eq!(parse("REPLACE"), Ok(Command::Replace));
        assert_eq!(parse("rep"), Ok(Command::Replace));
        assert_eq!(err("replace x"), "Usage: replace");
        assert_eq!(matches("re"), vec!["redo", "replace"]);
        assert_eq!(complete("rep"), "replace");
    }

    #[test]
    fn clipboard_commands_parse_without_arguments() {
        assert_eq!(parse("copy"), Ok(Command::Copy));
        assert_eq!(parse("cut"), Ok(Command::Cut));
        assert_eq!(parse("paste"), Ok(Command::Paste));
        assert_eq!(parse("selectall"), Ok(Command::SelectAll));
        assert_eq!(parse("COPY"), Ok(Command::Copy));
        assert_eq!(err("copy now"), "Usage: copy");
        assert_eq!(err("cut 3"), "Usage: cut");
        assert_eq!(err("paste x"), "Usage: paste");
        assert_eq!(err("selectall x"), "Usage: selectall");
    }

    #[test]
    fn new_names_change_which_prefixes_are_ambiguous() {
        // Unique prefixes of the new names.
        assert_eq!(parse("co"), Ok(Command::Copy));
        assert_eq!(parse("cu"), Ok(Command::Cut));
        assert_eq!(parse("pa"), Ok(Command::Paste));
        assert_eq!(parse("sel"), Ok(Command::SelectAll));
        // Still unique: these were before, and the new names do not share them.
        assert_eq!(parse("cl"), Ok(Command::Close));
        assert_eq!(parse("pr"), Ok(Command::Prev));
        assert_eq!(parse("sav"), Err("Ambiguous: save, saveas".to_string()));
        // No longer unique: one letter now matches more names.
        assert_eq!(err("c"), "Ambiguous: close, copy, cut");
        assert_eq!(err("p"), "Ambiguous: prev, paste");
        assert_eq!(err("s"), "Ambiguous: save, saveas, selectall");
        assert_eq!(parse("se"), Ok(Command::SelectAll));
    }

    #[test]
    fn clipboard_names_complete() {
        assert_eq!(complete("sel"), "selectall");
        assert_eq!(complete("pa"), "paste");
        assert_eq!(complete("c"), "c");
        assert_eq!(complete("co"), "copy");
        assert_eq!(complete("cu"), "cut");
        assert_eq!(matches("c"), vec!["close", "copy", "cut"]);
        assert_eq!(matches("p"), vec!["prev", "paste"]);
    }

    #[test]
    fn qreplace_parses_without_arguments_and_leaves_other_prefixes_alone() {
        assert_eq!(parse("qreplace"), Ok(Command::ReplaceAsk));
        assert_eq!(parse("qr"), Ok(Command::ReplaceAsk));
        assert_eq!(err("qreplace x"), "Usage: qreplace");
        assert_eq!(parse("q"), Ok(Command::Quit));
        assert_eq!(parse("qu"), Ok(Command::Quit));
        assert_eq!(parse("rep"), Ok(Command::Replace));
        assert_eq!(complete("qr"), "qreplace");
        assert_eq!(matches("q"), vec!["qreplace", "quit"]);
    }

    #[test]
    fn complete_saveas_adds_a_space() {
        assert_eq!(complete("savea"), "saveas ");
        assert_eq!(complete("sa"), "save");
        assert_eq!(complete("saveas"), "saveas ");
    }
}
