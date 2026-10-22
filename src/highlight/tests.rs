use super::*;
use std::path::Path;

fn letter(t: Token) -> char {
    match t {
        Token::Plain => '.',
        Token::Keyword => 'k',
        Token::Type => 't',
        Token::Str => 's',
        Token::Comment => 'c',
        Token::Number => 'n',
        Token::Meta => 'm',
    }
}

fn run(state: LineState, line: &str) -> (String, LineState) {
    let (tokens, next) = highlight_line(Language::Rust, state, line);
    assert_eq!(tokens.len(), line.chars().count(), "one token per char: {line:?}");
    (tokens.into_iter().map(letter).collect(), next)
}

fn render(line: &str) -> String {
    let (s, next) = run(LineState::Normal, line);
    assert_eq!(next, LineState::Normal, "state after {line:?}");
    s
}

fn all(c: char, line: &str) -> String {
    c.to_string().repeat(line.chars().count())
}

#[test]
fn from_path_recognizes_rs_only() {
    assert_eq!(Language::from_path(Path::new("main.rs")), Some(Language::Rust));
    assert_eq!(Language::from_path(Path::new("/a/b/lib.rs")), Some(Language::Rust));
    for p in ["README.md", "Makefile", "x.RS", "rs", ""] {
        assert_eq!(Language::from_path(Path::new(p)), None, "{p}");
    }
}

#[test]
fn default_state_is_normal() {
    assert_eq!(LineState::default(), LineState::Normal);
}

#[test]
fn keywords_and_self() {
    assert_eq!(render("let x = 5;"), "kkk.....n.");
    assert_eq!(render("fn main() {}"), "kk..........");
    assert_eq!(render("self.x = Self::new()"), "kkkk.....tttt.......");
}

#[test]
fn types_constants_and_identifiers() {
    assert_eq!(render("Vec<PathBuf>"), "ttt.ttttttt.");
    assert_eq!(render("MAX_SIZE foo"), all('.', "MAX_SIZE foo"));
    assert_eq!(render("A B"), "...");
}

#[test]
fn identifiers_containing_keywords_stay_plain() {
    for s in ["format letter mod_name", "fnord iffy", "_in x_for"] {
        assert_eq!(render(s), all('.', s), "{s}");
    }
}

#[test]
fn macros() {
    assert_eq!(render(r#"eprintln!("x")"#), "mmmmmmmmm.sss.");
    assert_eq!(render("vec![1]"), "mmmm.n.");
    assert_eq!(render("a != b"), "......");
    assert_eq!(render("a!=b"), "....");
}

#[test]
fn line_comments() {
    assert_eq!(render("x // hi"), "..ccccc");
    assert_eq!(render("/// doc"), "ccccccc");
    assert_eq!(render("//! mod"), "ccccccc");
    assert_eq!(render("let s = \"a\"; // x"), "kkk.....sss..cccc");
}

#[test]
fn comment_marker_inside_string_stays_string() {
    assert_eq!(render(r#""a // b""#), "ssssssss");
}

#[test]
fn quote_inside_comment_stays_comment() {
    let s = "// say \"hi";
    assert_eq!(render(s), all('c', s));
    let s = "/* \" */";
    assert_eq!(render(s), all('c', s));
}

#[test]
fn nested_block_comment_one_line() {
    assert_eq!(render("/* a /* b */ still */ x"), "ccccccccccccccccccccc..");
}

#[test]
fn nested_block_comment_carries_depth_over_lines() {
    let (r, st) = run(LineState::Normal, "a /* x");
    assert_eq!((r.as_str(), st), ("..cccc", LineState::BlockComment(1)));
    let (r, st) = run(st, "/* y");
    assert_eq!((r.as_str(), st), ("cccc", LineState::BlockComment(2)));
    let (r, st) = run(st, "*/ z");
    assert_eq!((r.as_str(), st), ("cccc", LineState::BlockComment(1)));
    let (r, st) = run(st, "*/ w");
    assert_eq!((r.as_str(), st), ("cc..", LineState::Normal));
}

#[test]
fn unterminated_block_comment_keeps_state() {
    let (r, st) = run(LineState::Normal, "/* open");
    assert_eq!((r.as_str(), st), ("ccccccc", LineState::BlockComment(1)));
    let (r, st) = run(LineState::BlockComment(1), "still open");
    assert_eq!((r.as_str(), st), ("cccccccccc", LineState::BlockComment(1)));
}

#[test]
fn strings_with_escapes() {
    assert_eq!(render(r#""a\"b""#), "ssssss");
    assert_eq!(render(r#""a\\" x"#), "sssss..");
}

#[test]
fn multi_line_string_carries_state() {
    let (r, st) = run(LineState::Normal, "let s = \"abc");
    assert_eq!((r.as_str(), st), ("kkk.....ssss", LineState::Str));
    let (r, st) = run(st, "def\" + 1;");
    assert_eq!((r.as_str(), st), ("ssss...n.", LineState::Normal));
}

#[test]
fn string_with_trailing_backslash_continues() {
    let (r, st) = run(LineState::Normal, "\"abc\\");
    assert_eq!((r.as_str(), st), ("sssss", LineState::Str));
}

#[test]
fn unterminated_string_keeps_state() {
    let (r, st) = run(LineState::Normal, "\"abc");
    assert_eq!((r.as_str(), st), ("ssss", LineState::Str));
    let (r, st) = run(LineState::Str, "still");
    assert_eq!((r.as_str(), st), ("sssss", LineState::Str));
}

#[test]
fn byte_strings() {
    assert_eq!(render(r#"b"hi""#), "sssss");
    assert_eq!(render("b + 1"), "....n");
}

#[test]
fn raw_strings() {
    assert_eq!(render(r#"r"x""#), "ssss");
    assert_eq!(render(r##"r#"a"b"#"##), "ssssssss");
    assert_eq!(render(r###"r##"a"#b"##"###), "sssssssssss");
    assert_eq!(render(r#"br"x""#), "sssss");
    assert_eq!(render(r##"br#"x"#"##), "sssssss");
}

#[test]
fn multi_line_raw_string_carries_hashes() {
    let (r, st) = run(LineState::Normal, "r#\"abc");
    assert_eq!((r.as_str(), st), ("ssssss", LineState::RawStr(1)));
    let (r, st) = run(st, "a\"b");
    assert_eq!((r.as_str(), st), ("sss", LineState::RawStr(1)));
    let (r, st) = run(st, "x\"y\"# z");
    assert_eq!((r.as_str(), st), ("sssss..", LineState::Normal));
}

#[test]
fn raw_string_needs_all_hashes_to_close() {
    let (r, st) = run(LineState::Normal, "r##\"a\"# b");
    assert_eq!((r.as_str(), st), ("sssssssss", LineState::RawStr(2)));
}

#[test]
fn raw_identifier_is_plain() {
    assert_eq!(render("r#type"), "......");
    assert_eq!(render("let r#type = 1;"), "kkk..........n.");
}

#[test]
fn char_literals() {
    assert_eq!(render("'a'"), "sss");
    assert_eq!(render(r"'\''"), "ssss");
    assert_eq!(render(r"'\n'"), "ssss");
    assert_eq!(render(r"'\u{1F600}'"), "sssssssssss");
    assert_eq!(render("'é'"), "sss");
    assert_eq!(render("b'x'"), "ssss");
    assert_eq!(render("let c = 'x';"), "kkk.....sss.");
}

#[test]
fn lifetimes_and_labels() {
    assert_eq!(render("<'a>"), ".mm.");
    assert_eq!(render("&'static str"), ".mmmmmmm....");
    assert_eq!(render("'_"), "mm");
    assert_eq!(render("'outer: loop"), "mmmmmm..kkkk");
}

#[test]
fn numbers() {
    assert_eq!(render("42"), "nn");
    assert_eq!(render("1_000"), "nnnnn");
    assert_eq!(render("0xFFu8"), "nnnnnn");
    assert_eq!(render("0b1010"), "nnnnnn");
    assert_eq!(render("0o777"), "nnnnn");
    assert_eq!(render("1.5"), "nnn");
    assert_eq!(render("1e10"), "nnnn");
    assert_eq!(render("1.5e-3f32"), "nnnnnnnnn");
    assert_eq!(render("x1 = 2"), ".....n");
}

#[test]
fn ranges_and_method_calls_are_not_floats() {
    assert_eq!(render("1..2"), "n..n");
    assert_eq!(render("1.max(2)"), "n.....n.");
}

#[test]
fn attributes() {
    for s in ["#[derive(Debug, Clone)]", "#![allow(dead_code)]", "#[cfg(any(a, b))]"] {
        assert_eq!(render(s), all('m', s), "{s}");
    }
    assert_eq!(render("#[test] fn"), "mmmmmmm.kk");
    assert_eq!(render("#[cfg(a"), "mmmmmmm");
    assert_eq!(render("# x"), "...");
}

#[test]
fn attribute_brackets_inside_strings_do_not_end_it() {
    let attr = r#"#[doc = "a]b"]"#;
    assert_eq!(render(&format!("{attr} fn")), format!("{}.kk", all('m', attr)));
    let attr = r#"#[doc = "a\"]b"]"#;
    assert_eq!(render(&format!("{attr} x")), format!("{}..", all('m', attr)));
    let attr = r#"#![doc = "[", x]"#;
    assert_eq!(render(&format!("{attr} fn")), format!("{}.kk", all('m', attr)));
    // An unterminated string inside an attribute: Meta to the end of this
    // line only; the next line starts Normal (render checks the state).
    let s = r#"#[doc = "a] fn x"#;
    assert_eq!(render(s), all('m', s));
    let s = r#"#[doc = "a\"#;
    assert_eq!(render(s), all('m', s));
}

#[test]
fn unicode_gives_one_token_per_char() {
    let s = "\"h\u{e9}llo \u{1F600} \u{4e16}\u{754c}\"";
    assert_eq!(render(s), all('s', s));
    let s = "// \u{1F600} \u{4e16}\u{754c}";
    assert_eq!(render(s), all('c', s));
    assert_eq!(render("let \u{4e16}\u{754c} = 1;"), "kkk......n.");
}

#[test]
fn empty_line_keeps_state() {
    for st in [
        LineState::Normal,
        LineState::BlockComment(2),
        LineState::Str,
        LineState::RawStr(3),
    ] {
        let (tokens, next) = highlight_line(Language::Rust, st, "");
        assert!(tokens.is_empty());
        assert_eq!(next, st);
    }
}

#[test]
fn never_panics_on_odd_input() {
    let big = [
        "a".repeat(100_000),
        "'".repeat(100_000),
        "\"".repeat(100_000),
        "r#".repeat(50_000),
        "r".to_string() + &"#".repeat(100_000),
        "/*".repeat(50_000),
        "*/".repeat(50_000),
        "#[".repeat(50_000),
        "0x".repeat(50_000),
        "'\\a".repeat(33_000),
        "\\".repeat(100_000),
    ];
    let mut cases: Vec<String> = [
        "'", "\"", "r#", "r#\"", "r", "br", "b", "/*", "*/", "\\", "0x", "1e", "1e-", "1.", "#[", "#![", "#",
        "b'", "'\\", "'\\u{", "\u{301}", "!", "a!", "a!=", "\u{1F600}", "r##\"", "\"\\",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    cases.extend(big);
    let states = [
        LineState::Normal,
        LineState::BlockComment(1),
        LineState::BlockComment(u32::MAX),
        LineState::Str,
        LineState::RawStr(0),
        LineState::RawStr(2),
        LineState::RawStr(u32::MAX),
    ];
    for case in &cases {
        for st in states {
            let (tokens, _) = highlight_line(Language::Rust, st, case);
            assert_eq!(tokens.len(), case.chars().count());
        }
    }
}

#[test]
fn state_carries_over_many_lines() {
    let program = [
        "/// Doc",
        "fn main() {",
        "    let s = \"a",
        "b\"; /* c",
        "d */ let n = 1;",
        "}",
    ];
    let expected = [
        "ccccccc",
        "kk.........",
        "....kkk.....ss",
        "ss..cccc",
        "cccc.kkk.....n.",
        ".",
    ];
    let mut st = LineState::Normal;
    for (line, want) in program.iter().zip(expected) {
        let (got, next) = run(st, line);
        assert_eq!(got, want, "{line:?}");
        st = next;
    }
    assert_eq!(st, LineState::Normal);
}
