//! Regex -> AST parser. Supports literals, concatenation, alternation
//! (`|`), quantifiers (`*`, `+`, `?`, `{m,n}`), capturing and non-capturing
//! groups (`(...)`, `(?:...)`), character classes (`[...]`, `.`, `\d`/`\w`/
//! `\s` and negations), and anchors (`^`, `$`). Malformed input returns a
//! positioned `ParseError` rather than panicking.

use std::fmt;

/// A single `[...]` character class: a set of inclusive char ranges,
/// optionally negated (`[^...]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharClass {
    pub negated: bool,
    pub ranges: Vec<(char, char)>,
}

impl CharClass {
    /// Does `c` fall inside this class, honoring negation?
    pub fn matches(&self, c: char) -> bool {
        let in_ranges = self.ranges.iter().any(|&(lo, hi)| c >= lo && c <= hi);
        in_ranges != self.negated
    }
}

/// A parsed regular expression, as a tree of matchable nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    /// Matches nothing but the empty string.
    Empty,
    /// A single literal character.
    Literal(char),
    /// `[a-z]`, `[^0-9]`, `.`, `\d`, `\w`, `\s` (and negations) — a set of
    /// characters to match against one input position.
    CharClass(CharClass),
    /// `^` — matches only at the start of the input.
    AnchorStart,
    /// `$` — matches only at the end of the input.
    AnchorEnd,
    /// `ab` — match each node in sequence.
    Concat(Vec<Ast>),
    /// `a|b` — match any one of the alternatives.
    Alternation(Vec<Ast>),
    /// `a*`, `a+`, `a?`, `a{m,n}` — repeat `node` between `min` and `max`
    /// times (`max = None` means unbounded).
    Repeat {
        node: Box<Ast>,
        min: u32,
        max: Option<u32>,
    },
    /// `(a)` — a capturing or non-capturing group.
    Group(Box<Ast>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Ranges for the `\d`/`\w`/`\s` shorthand classes and their negations
/// (`\D`/`\W`/`\S`), shared between a top-level escape (`parse_escape`) and
/// an escape inside a `[...]` character class (`parse_char_class`). Returns
/// `(negated, ranges)`, or `None` if `c` isn't a shorthand-class letter.
fn shorthand_class(c: char) -> Option<(bool, Vec<(char, char)>)> {
    match c {
        'd' => Some((false, vec![('0', '9')])),
        'D' => Some((true, vec![('0', '9')])),
        'w' => Some((false, vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')])),
        'W' => Some((true, vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')])),
        's' => Some((
            false,
            vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
        )),
        'S' => Some((
            true,
            vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
        )),
        _ => None,
    }
}

/// Recursive-descent parser over a regex pattern's chars.
///
/// Grammar:
/// ```text
/// alternation := concat ('|' concat)*
/// concat      := quantified_atom*
/// quantified_atom := atom ('*' | '+' | '?' | '{' m (',' n?)? '}')?
/// atom        := literal | '(' ('?:')? alternation ')' | '[' class_item* ']'
///              | '^' | '$' | '.' | '\' escape
/// ```
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(pattern: &str) -> Self {
        Parser {
            chars: pattern.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            position: self.pos,
        }
    }

    fn parse_alternation(&mut self) -> Result<Ast, ParseError> {
        let mut branches = vec![self.parse_concat()?];
        while self.peek() == Some('|') {
            self.advance();
            branches.push(self.parse_concat()?);
        }
        if branches.len() == 1 {
            Ok(branches.into_iter().next().unwrap())
        } else {
            Ok(Ast::Alternation(branches))
        }
    }

    fn parse_concat(&mut self) -> Result<Ast, ParseError> {
        let mut nodes = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            nodes.push(self.parse_quantified_atom()?);
        }
        match nodes.len() {
            0 => Ok(Ast::Empty),
            1 => Ok(nodes.into_iter().next().unwrap()),
            _ => Ok(Ast::Concat(nodes)),
        }
    }

    /// An atom optionally followed by a `*`/`+`/`?` quantifier.
    fn parse_quantified_atom(&mut self) -> Result<Ast, ParseError> {
        let atom = self.parse_atom()?;
        match self.peek() {
            Some('*') => {
                self.advance();
                Ok(Ast::Repeat {
                    node: Box::new(atom),
                    min: 0,
                    max: None,
                })
            }
            Some('+') => {
                self.advance();
                Ok(Ast::Repeat {
                    node: Box::new(atom),
                    min: 1,
                    max: None,
                })
            }
            Some('?') => {
                self.advance();
                Ok(Ast::Repeat {
                    node: Box::new(atom),
                    min: 0,
                    max: Some(1),
                })
            }
            Some('{') => self.parse_bounded_repeat(atom),
            _ => Ok(atom),
        }
    }

    /// `{m}`, `{m,}`, or `{m,n}` following an already-parsed atom.
    fn parse_bounded_repeat(&mut self, atom: Ast) -> Result<Ast, ParseError> {
        self.advance(); // consume '{'
        let min = self.parse_number()?;
        let max = if self.peek() == Some(',') {
            self.advance();
            if self.peek() == Some('}') {
                None
            } else {
                Some(self.parse_number()?)
            }
        } else {
            Some(min)
        };
        if self.advance() != Some('}') {
            return Err(self.error("malformed repeat: expected '}'"));
        }
        if let Some(max) = max {
            if max < min {
                return Err(self.error("repeat max must be >= min"));
            }
        }
        Ok(Ast::Repeat {
            node: Box::new(atom),
            min,
            max,
        })
    }

    fn parse_number(&mut self) -> Result<u32, ParseError> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
        if self.pos == start {
            return Err(self.error("expected a number in repeat bounds"));
        }
        let digits: String = self.chars[start..self.pos].iter().collect();
        digits
            .parse()
            .map_err(|_| self.error("repeat bound is not a valid number"))
    }

    fn parse_atom(&mut self) -> Result<Ast, ParseError> {
        match self.peek() {
            None => Err(self.error("expected an atom but found end of pattern")),
            Some('(') => {
                self.advance();
                if self.peek() == Some('?') {
                    self.advance();
                    // `(?:...)` — a non-capturing group. This crate never
                    // tracks captures either way, so it parses identically
                    // to `(...)`; anything else after `(?` (lookaround,
                    // inline flags) isn't supported.
                    if self.advance() != Some(':') {
                        return Err(self.error(
                            "unsupported group syntax: only non-capturing groups (?:...) \
                             are supported (no lookaround or inline flags)",
                        ));
                    }
                }
                let inner = self.parse_alternation()?;
                if self.advance() != Some(')') {
                    return Err(self.error("unbalanced parenthesis: expected ')'"));
                }
                Ok(Ast::Group(Box::new(inner)))
            }
            Some(')') => Err(self.error("unexpected ')' with no matching '('")),
            Some('*') | Some('+') | Some('?') => {
                Err(self.error("dangling quantifier with no preceding atom"))
            }
            Some('[') => self.parse_char_class(),
            Some('^') => {
                self.advance();
                Ok(Ast::AnchorStart)
            }
            Some('$') => {
                self.advance();
                Ok(Ast::AnchorEnd)
            }
            Some('.') => {
                self.advance();
                // Matches any character except '\n', matching the
                // conventional (non-dotall) behavior of PCRE/JS/Python's
                // `.` — an unconditionally-matches-everything `.` can never
                // be forced to fail a fullmatch, which breaks worst-case
                // generation for any nested-quantifier pattern built on it
                // (e.g. `(.+)+`, a common real-world ReDoS shape).
                Ok(Ast::CharClass(CharClass {
                    negated: true,
                    ranges: vec![('\n', '\n')],
                }))
            }
            Some('\\') => self.parse_escape(),
            Some(c) => {
                self.advance();
                Ok(Ast::Literal(c))
            }
        }
    }

    /// `\d`, `\w`, `\s` (and negated `\D`, `\W`, `\S`) shorthand classes,
    /// plus `\<metachar>` for an escaped literal like `\.` or `\+`.
    fn parse_escape(&mut self) -> Result<Ast, ParseError> {
        self.advance(); // consume '\'
        let c = self
            .advance()
            .ok_or_else(|| self.error("dangling escape at end of pattern"))?;
        match shorthand_class(c) {
            Some((negated, ranges)) => Ok(Ast::CharClass(CharClass { negated, ranges })),
            None => Ok(Ast::Literal(c)),
        }
    }

    /// `[abc]`, `[^abc]`, `[a-z0-9]` — a bracketed set of chars/ranges.
    fn parse_char_class(&mut self) -> Result<Ast, ParseError> {
        self.advance(); // consume '['
        let negated = if self.peek() == Some('^') {
            self.advance();
            true
        } else {
            false
        };
        let mut ranges = Vec::new();
        loop {
            match self.peek() {
                None => return Err(self.error("unbalanced character class: expected ']'")),
                Some(']') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance(); // consume '\'
                    let escaped = self
                        .advance()
                        .ok_or_else(|| self.error("dangling escape in character class"))?;
                    match shorthand_class(escaped) {
                        Some((false, class_ranges)) => ranges.extend(class_ranges),
                        Some((true, _)) => {
                            return Err(self.error(
                                "negated shorthand classes like \\D are not supported inside \
                                 a character class",
                            ));
                        }
                        // \], \-, \\, \^, or any other escaped literal.
                        None => ranges.push((escaped, escaped)),
                    }
                }
                Some(_) => {
                    let lo = self.advance().unwrap();
                    if self.peek() == Some('-') && self.chars.get(self.pos + 1) != Some(&']') {
                        self.advance(); // consume '-'
                        let hi = self
                            .advance()
                            .ok_or_else(|| self.error("unbalanced character class"))?;
                        if hi < lo {
                            return Err(self.error("character class range is out of order"));
                        }
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
            }
        }
        if ranges.is_empty() {
            return Err(self.error("character class must not be empty"));
        }
        Ok(Ast::CharClass(CharClass { negated, ranges }))
    }
}

/// Parse a regex pattern into an [`Ast`].
pub fn parse(pattern: &str) -> Result<Ast, ParseError> {
    let mut parser = Parser::new(pattern);
    let ast = parser.parse_alternation()?;
    if parser.pos != parser.chars.len() {
        return Err(parser.error("unexpected trailing input"));
    }
    Ok(ast)
}

const METACHARS: &str = ".^$*+?()[]{}|\\";

/// Render `ast` back into regex source, escaping literals that would
/// otherwise be read as metacharacters. Used by [`crate::rewrite`] to turn a
/// transformed AST back into a pattern string the caller (and the WASM
/// bridge) can re-parse and re-run.
pub fn to_pattern(ast: &Ast) -> String {
    match ast {
        Ast::Empty => String::new(),
        Ast::Literal(c) => render_literal(*c),
        Ast::CharClass(class) => render_char_class(class),
        Ast::AnchorStart => "^".to_string(),
        Ast::AnchorEnd => "$".to_string(),
        Ast::Concat(nodes) => nodes.iter().map(render_concat_member).collect(),
        Ast::Alternation(branches) => branches
            .iter()
            .map(to_pattern)
            .collect::<Vec<_>>()
            .join("|"),
        Ast::Group(inner) => format!("({})", to_pattern(inner)),
        Ast::Repeat { node, min, max } => {
            format!(
                "{}{}",
                render_repeat_operand(node),
                render_quantifier(*min, *max)
            )
        }
    }
}

fn render_literal(c: char) -> String {
    if METACHARS.contains(c) {
        format!("\\{c}")
    } else {
        c.to_string()
    }
}

fn render_char_class(class: &CharClass) -> String {
    if class.negated && class.ranges == [('\n', '\n')] {
        return ".".to_string();
    }
    let mut out = String::from("[");
    if class.negated {
        out.push('^');
    }
    for &(lo, hi) in &class.ranges {
        if lo == hi {
            out.push(lo);
        } else {
            out.push(lo);
            out.push('-');
            out.push(hi);
        }
    }
    out.push(']');
    out
}

/// A concat member needs grouping only if it's an alternation — otherwise
/// it would swallow the rest of the sequence as extra branches.
fn render_concat_member(ast: &Ast) -> String {
    match ast {
        Ast::Alternation(_) => format!("({})", to_pattern(ast)),
        other => to_pattern(other),
    }
}

/// A repeat's operand needs grouping if it's an alternation or a multi-node
/// concat — otherwise the quantifier would bind to just the last atom.
fn render_repeat_operand(ast: &Ast) -> String {
    match ast {
        Ast::Alternation(_) => format!("({})", to_pattern(ast)),
        Ast::Concat(nodes) if nodes.len() > 1 => format!("({})", to_pattern(ast)),
        other => to_pattern(other),
    }
}

fn render_quantifier(min: u32, max: Option<u32>) -> String {
    match (min, max) {
        (0, None) => "*".to_string(),
        (1, None) => "+".to_string(),
        (0, Some(1)) => "?".to_string(),
        (min, None) => format!("{{{min},}}"),
        (min, Some(max)) if min == max => format!("{{{min}}}"),
        (min, Some(max)) => format!("{{{min},{max}}}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pattern_parses_to_empty_node() {
        assert_eq!(parse("").unwrap(), Ast::Empty);
    }

    #[test]
    fn single_literal_parses() {
        assert_eq!(parse("a").unwrap(), Ast::Literal('a'));
    }

    #[test]
    fn concatenation_parses_as_a_sequence() {
        assert_eq!(
            parse("ab").unwrap(),
            Ast::Concat(vec![Ast::Literal('a'), Ast::Literal('b')])
        );
    }

    #[test]
    fn alternation_parses_branches() {
        assert_eq!(
            parse("a|b").unwrap(),
            Ast::Alternation(vec![Ast::Literal('a'), Ast::Literal('b')])
        );
    }

    #[test]
    fn group_wraps_its_contents() {
        assert_eq!(
            parse("(a|b)").unwrap(),
            Ast::Group(Box::new(Ast::Alternation(vec![
                Ast::Literal('a'),
                Ast::Literal('b')
            ])))
        );
    }

    #[test]
    fn unbalanced_open_paren_is_a_parse_error() {
        let err = parse("(a").unwrap_err();
        assert_eq!(err.position, 2);
    }

    #[test]
    fn non_capturing_group_parses_like_a_capturing_group() {
        assert_eq!(parse("(?:a|b)").unwrap(), parse("(a|b)").unwrap());
    }

    #[test]
    fn non_capturing_group_quantifier_still_binds_to_the_group() {
        assert_eq!(
            parse("(?:ab)+").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Group(Box::new(Ast::Concat(vec![
                    Ast::Literal('a'),
                    Ast::Literal('b')
                ])))),
                min: 1,
                max: None,
            }
        );
    }

    #[test]
    fn unsupported_group_extension_is_a_parse_error() {
        assert!(parse("(?=a)").is_err());
        assert!(parse("(?!a)").is_err());
        assert!(parse("(?i)a").is_err());
    }

    #[test]
    fn unmatched_close_paren_is_a_parse_error() {
        assert!(parse("a)").is_err());
    }

    #[test]
    fn star_quantifier_allows_zero_or_more() {
        assert_eq!(
            parse("a*").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 0,
                max: None,
            }
        );
    }

    #[test]
    fn plus_quantifier_requires_at_least_one() {
        assert_eq!(
            parse("a+").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 1,
                max: None,
            }
        );
    }

    #[test]
    fn question_quantifier_is_zero_or_one() {
        assert_eq!(
            parse("a?").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 0,
                max: Some(1),
            }
        );
    }

    #[test]
    fn quantifier_binds_to_a_group() {
        assert_eq!(
            parse("(ab)+").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Group(Box::new(Ast::Concat(vec![
                    Ast::Literal('a'),
                    Ast::Literal('b')
                ])))),
                min: 1,
                max: None,
            }
        );
    }

    #[test]
    fn dangling_quantifier_is_a_parse_error() {
        assert!(parse("*a").is_err());
        assert!(parse("+").is_err());
    }

    #[test]
    fn exact_count_repeat() {
        assert_eq!(
            parse("a{3}").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 3,
                max: Some(3),
            }
        );
    }

    #[test]
    fn open_ended_repeat() {
        assert_eq!(
            parse("a{2,}").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 2,
                max: None,
            }
        );
    }

    #[test]
    fn bounded_range_repeat() {
        assert_eq!(
            parse("a{1,20}").unwrap(),
            Ast::Repeat {
                node: Box::new(Ast::Literal('a')),
                min: 1,
                max: Some(20),
            }
        );
    }

    #[test]
    fn malformed_bounded_repeat_is_a_parse_error() {
        assert!(parse("a{2,1}").is_err());
        assert!(parse("a{}").is_err());
        assert!(parse("a{2").is_err());
    }

    #[test]
    fn char_class_of_single_chars() {
        assert_eq!(
            parse("[abc]").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![('a', 'a'), ('b', 'b'), ('c', 'c')],
            })
        );
    }

    #[test]
    fn char_class_range() {
        assert_eq!(
            parse("[a-z]").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![('a', 'z')],
            })
        );
    }

    #[test]
    fn char_class_negation() {
        assert_eq!(
            parse("[^0-9]").unwrap(),
            Ast::CharClass(CharClass {
                negated: true,
                ranges: vec![('0', '9')],
            })
        );
    }

    #[test]
    fn char_class_trailing_dash_is_literal() {
        assert_eq!(
            parse("[a-]").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![('a', 'a'), ('-', '-')],
            })
        );
    }

    #[test]
    fn unbalanced_char_class_is_a_parse_error() {
        assert!(parse("[abc").is_err());
    }

    #[test]
    fn char_class_embeds_a_shorthand_class() {
        // [\d.] — very common in real-world patterns (e.g. "[\w.-]+" for
        // a hostname segment) — merges \d's ranges into the surrounding
        // class rather than treating '\' and 'd' as two literal chars.
        assert_eq!(
            parse(r"[\d.]").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![('0', '9'), ('.', '.')],
            })
        );
    }

    #[test]
    fn char_class_embeds_multiple_shorthand_classes() {
        let ast = parse(r"[\w\s]").unwrap();
        let Ast::CharClass(class) = ast else {
            panic!("expected a CharClass");
        };
        assert!(class.matches('a'));
        assert!(class.matches('_'));
        assert!(class.matches(' '));
        assert!(!class.matches('!'));
    }

    #[test]
    fn char_class_escaped_bracket_is_a_literal_member() {
        assert_eq!(
            parse(r"[\]a]").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![(']', ']'), ('a', 'a')],
            })
        );
    }

    #[test]
    fn negated_shorthand_class_inside_char_class_is_a_parse_error() {
        assert!(parse(r"[\D]").is_err());
        assert!(parse(r"[a\W]").is_err());
    }

    #[test]
    fn dangling_escape_in_char_class_is_a_parse_error() {
        assert!(parse("[a\\").is_err());
    }

    #[test]
    fn empty_char_class_is_a_parse_error() {
        assert!(parse("[]").is_err());
    }

    #[test]
    fn anchors_parse_as_dedicated_nodes() {
        assert_eq!(
            parse("^a$").unwrap(),
            Ast::Concat(vec![Ast::AnchorStart, Ast::Literal('a'), Ast::AnchorEnd])
        );
    }

    #[test]
    fn dot_matches_any_char_except_newline() {
        assert_eq!(
            parse(".").unwrap(),
            Ast::CharClass(CharClass {
                negated: true,
                ranges: vec![('\n', '\n')],
            })
        );
    }

    #[test]
    fn digit_shorthand_class() {
        assert_eq!(
            parse(r"\d").unwrap(),
            Ast::CharClass(CharClass {
                negated: false,
                ranges: vec![('0', '9')],
            })
        );
    }

    #[test]
    fn negated_word_shorthand_class() {
        assert_eq!(
            parse(r"\W").unwrap(),
            Ast::CharClass(CharClass {
                negated: true,
                ranges: vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
            })
        );
    }

    #[test]
    fn escaped_metachar_is_a_literal() {
        assert_eq!(parse(r"\.").unwrap(), Ast::Literal('.'));
        assert_eq!(parse(r"\+").unwrap(), Ast::Literal('+'));
    }

    #[test]
    fn dangling_escape_is_a_parse_error() {
        assert!(parse("\\").is_err());
    }

    #[test]
    fn to_pattern_round_trips_through_parse() {
        for pattern in [
            "a", "ab", "a|b", "a*", "a+", "a?", "a{3}", "a{2,}", "a{1,20}", "(ab)+", "(a|b)*",
            "(?:ab)+",
            "[a-z]+", "^a$",
        ] {
            let ast = parse(pattern).unwrap();
            let rendered = to_pattern(&ast);
            assert_eq!(
                parse(&rendered).unwrap(),
                ast,
                "expected {pattern} to round-trip via {rendered}"
            );
        }
    }

    #[test]
    fn to_pattern_escapes_metacharacters() {
        assert_eq!(to_pattern(&Ast::Literal('.')), "\\.");
        assert_eq!(to_pattern(&Ast::Literal('+')), "\\+");
        assert_eq!(to_pattern(&Ast::Literal('a')), "a");
    }

    #[test]
    fn to_pattern_renders_dot_from_negated_newline_class() {
        let ast = Ast::CharClass(CharClass {
            negated: true,
            ranges: vec![('\n', '\n')],
        });
        assert_eq!(to_pattern(&ast), ".");
    }

    #[test]
    fn dot_does_not_match_newline() {
        let dot = parse(".").unwrap();
        let Ast::CharClass(class) = dot else {
            panic!("expected a CharClass");
        };
        assert!(!class.matches('\n'));
        assert!(class.matches('a'));
        assert!(class.matches('!'));
    }

    #[test]
    fn char_class_matches_honors_negation() {
        let digits = CharClass {
            negated: false,
            ranges: vec![('0', '9')],
        };
        assert!(digits.matches('5'));
        assert!(!digits.matches('a'));

        let not_digits = CharClass {
            negated: true,
            ranges: vec![('0', '9')],
        };
        assert!(!not_digits.matches('5'));
        assert!(not_digits.matches('a'));
    }
}
