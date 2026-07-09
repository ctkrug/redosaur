//! Regex -> AST parser. Placeholder shape for the ReDoSaur core engine;
//! the full grammar (concatenation, alternation, quantifiers, groups,
//! character classes) is built out in docs/BACKLOG.md Epic 1.

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

/// Recursive-descent parser over a regex pattern's chars.
///
/// Grammar (quantifiers, character classes, anchors land in follow-up
/// commits per docs/BACKLOG.md 1.2):
/// ```text
/// alternation := concat ('|' concat)*
/// concat      := atom*
/// atom        := literal | '(' alternation ')'
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
            nodes.push(self.parse_atom()?);
        }
        match nodes.len() {
            0 => Ok(Ast::Empty),
            1 => Ok(nodes.into_iter().next().unwrap()),
            _ => Ok(Ast::Concat(nodes)),
        }
    }

    fn parse_atom(&mut self) -> Result<Ast, ParseError> {
        match self.peek() {
            None => Err(self.error("expected an atom but found end of pattern")),
            Some('(') => {
                self.advance();
                let inner = self.parse_alternation()?;
                if self.advance() != Some(')') {
                    return Err(self.error("unbalanced parenthesis: expected ')'"));
                }
                Ok(Ast::Group(Box::new(inner)))
            }
            Some(')') => Err(self.error("unexpected ')' with no matching '('")),
            Some(c) => {
                self.advance();
                Ok(Ast::Literal(c))
            }
        }
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
}
