//! Splitting a command line into simple commands, the way a shell would.
//!
//! Two consumers need the same answer to "which words run as which command":
//! the interceptor that finds `git push` (`invariant.rs`) and the classifier
//! that decides whether a command changes nothing (`readonly.rs`). Both fail
//! the same way when the split is wrong: a `;` taken for part of a quoted
//! string runs unseen.
//!
//! The core does not know which shell will read a line, and the same bytes can
//! mean different things to two shells. A backslash that escapes a quote in
//! POSIX opens a string in PowerShell, and that difference is exactly where a
//! hidden command would hide. So a line is parsed under every dialect the core
//! can hand it to, and a verdict has to hold under all of them. The core runs
//! commands through PowerShell on Windows and through `sh` elsewhere; cmd is
//! not one of them, and `cmd /c` is not something the classifier accepts.

/// The shells a line may be read by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Posix,
    PowerShell,
}

pub(crate) const DIALECTS: [Dialect; 2] = [Dialect::Posix, Dialect::PowerShell];

/// One word, with its quotes taken off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Word {
    pub(crate) text: String,
    /// The shell would expand, glob, redirect or escape something in this word
    /// (`$x`, `*`, `>`, a backtick). Its text is then not what will run, and
    /// nothing can be concluded from it.
    pub(crate) hazard: bool,
}

/// A simple command: the words between separators.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Simple {
    pub(crate) words: Vec<Word>,
    /// Some word in it is hazardous, or the command substitutes output into
    /// it, or it redirects.
    pub(crate) hazard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) commands: Vec<Simple>,
    /// Every quote was closed. Otherwise the line means something other than
    /// what the split shows, and no caller should trust the split.
    pub(crate) closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Unquoted,
    Double,
    Single,
}

pub(crate) fn parse(line: &str, dialect: Dialect) -> Line {
    let chars: Vec<char> = line.chars().collect();
    let mut p = Parser::default();
    let mut ctx = Ctx::Unquoted;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        i += 1;

        match (ctx, c) {
            (Ctx::Single, '\'') => ctx = Ctx::Unquoted,
            (Ctx::Single, _) => p.push(c, false),

            // Backslash is an escape in POSIX only. Under PowerShell a Windows
            // path keeps its separators, and reading it as an escape here would
            // join words that are two.
            (Ctx::Unquoted, '\\') if dialect == Dialect::Posix => match next {
                Some('\n') => i += 1,
                Some(n) => {
                    p.push(n, false);
                    i += 1;
                }
                None => p.push(c, false),
            },
            (Ctx::Double, '\\') if dialect == Dialect::Posix => match next {
                Some(n @ ('"' | '\\' | '$' | '`')) => {
                    p.push(n, false);
                    i += 1;
                }
                Some('\n') => i += 1,
                _ => p.push(c, false),
            },

            // PowerShell escapes with a backtick, inside quotes as well. The
            // escaped character is taken literally, and the word is marked: the
            // escape is a thing this classifier does not model.
            (Ctx::Unquoted | Ctx::Double, '`') if dialect == Dialect::PowerShell => match next {
                Some('\n') => i += 1,
                Some(n) => {
                    p.push(n, true);
                    i += 1;
                }
                None => p.push(c, true),
            },

            (Ctx::Unquoted, '\'') => {
                p.ensure_word();
                ctx = Ctx::Single;
            }
            (Ctx::Unquoted, '"') => {
                p.ensure_word();
                ctx = Ctx::Double;
            }
            (Ctx::Double, '"') => ctx = Ctx::Unquoted,

            (Ctx::Unquoted, ' ' | '\t' | '\r') => p.end_word(),
            (Ctx::Unquoted, '\n' | ';' | '|' | '&' | '(' | ')' | '{' | '}') => p.end_simple(),

            // Command substitution: the output becomes an argument of the
            // command it sits in, and the substituted text is a command of its
            // own. Both are cut off here, so both are seen.
            (Ctx::Unquoted | Ctx::Double, '`') if dialect == Dialect::Posix => {
                p.mark_hazard();
                p.end_simple();
            }

            (ctx, c) => p.push(c, hazardous(c, ctx, dialect)),
        }
    }

    p.end_simple();
    Line {
        commands: p.commands,
        closed: ctx == Ctx::Unquoted,
    }
}

/// Characters that do something to the word they sit in when no quote
/// protects them. `*` and `?` glob, `<` and `>` redirect, `$` expands; inside
/// double quotes only `$` still expands. `@` splats in PowerShell.
fn hazardous(c: char, ctx: Ctx, dialect: Dialect) -> bool {
    match ctx {
        Ctx::Single => false,
        Ctx::Double => matches!(c, '$' | '`'),
        Ctx::Unquoted => {
            matches!(c, '$' | '*' | '?' | '[' | ']' | '<' | '>')
                || (c == '@' && dialect == Dialect::PowerShell)
        }
    }
}

#[derive(Default)]
struct Parser {
    commands: Vec<Simple>,
    simple: Simple,
    word: Option<Word>,
}

impl Parser {
    fn push(&mut self, c: char, hazard: bool) {
        let word = self.word.get_or_insert_with(Word::default);
        word.text.push(c);
        if hazard {
            word.hazard = true;
            self.simple.hazard = true;
        }
    }

    /// A quote opens a word even when it is empty: `""` is an argument.
    fn ensure_word(&mut self) {
        self.word.get_or_insert_with(Word::default);
    }

    fn mark_hazard(&mut self) {
        self.simple.hazard = true;
    }

    fn end_word(&mut self) {
        if let Some(word) = self.word.take() {
            if word.hazard {
                self.simple.hazard = true;
            }
            self.simple.words.push(word);
        }
    }

    fn end_simple(&mut self) {
        self.end_word();
        let simple = std::mem::take(&mut self.simple);
        // A command with nothing in it is no command. One with a hazard is kept:
        // the hazard is what the caller needs to see.
        if !simple.words.is_empty() || simple.hazard {
            self.commands.push(simple);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str, dialect: Dialect) -> Vec<Vec<String>> {
        parse(line, dialect)
            .commands
            .into_iter()
            .map(|s| s.words.into_iter().map(|w| w.text).collect())
            .collect()
    }

    #[test]
    fn separators_split_simple_commands() {
        let split = vec![
            vec!["ls".to_string()],
            vec!["rm".to_string(), "x".to_string()],
        ];
        for d in DIALECTS {
            assert_eq!(words("ls; rm x", d), split, "{d:?}");
            assert_eq!(words("ls && rm x", d), split, "{d:?}");
            assert_eq!(words("ls | rm x", d), split, "{d:?}");
        }
    }

    #[test]
    fn a_separator_inside_quotes_is_text() {
        for d in DIALECTS {
            assert_eq!(words("echo 'a; b'", d), vec![vec!["echo", "a; b"]], "{d:?}");
            assert_eq!(
                words("echo \"a; b\"", d),
                vec![vec!["echo", "a; b"]],
                "{d:?}"
            );
        }
    }

    #[test]
    fn a_backslash_escapes_only_where_the_shell_does() {
        // POSIX: the quote is escaped, so the `;` is outside any string.
        let posix = words("echo \\\"; rm x; echo \\\"", Dialect::Posix);
        assert!(posix.iter().any(|c| c == &["rm", "x"]), "{posix:?}");
        // PowerShell: the quote opens a string, which hides the `;` from the split.
        let ps = words("echo \\\"; rm x; echo \\\"", Dialect::PowerShell);
        assert!(!ps.iter().any(|c| c == &["rm", "x"]), "{ps:?}");
    }

    #[test]
    fn a_windows_path_keeps_its_backslashes() {
        assert_eq!(
            words("C:\\bin\\git.exe status", Dialect::PowerShell),
            vec![vec!["C:\\bin\\git.exe", "status"]]
        );
    }

    #[test]
    fn empty_quotes_are_an_argument() {
        assert_eq!(
            words("echo \"\" x", Dialect::PowerShell),
            vec![vec!["echo", "", "x"]]
        );
    }

    #[test]
    fn an_unclosed_quote_is_reported() {
        assert!(!parse("echo \"abc", Dialect::Posix).closed);
        assert!(parse("echo \"abc\"", Dialect::Posix).closed);
    }

    #[test]
    fn expansion_marks_the_command() {
        let line = parse("echo $HOME", Dialect::Posix);
        assert!(line.commands[0].hazard);
        let quiet = parse("echo '$HOME'", Dialect::Posix);
        assert!(!quiet.commands[0].hazard);
    }
}
