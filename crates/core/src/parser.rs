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

/// Parse a regex pattern into an [`Ast`].
///
/// Currently supports the empty pattern and single-character literals
/// only; the rest of the grammar lands across docs/BACKLOG.md Epic 1.
pub fn parse(pattern: &str) -> Result<Ast, ParseError> {
    if pattern.is_empty() {
        return Ok(Ast::Empty);
    }
    let mut chars = pattern.chars();
    let first = chars.next().unwrap();
    if chars.next().is_some() {
        return Err(ParseError {
            message: "multi-character patterns are not yet supported".into(),
            position: 1,
        });
    }
    Ok(Ast::Literal(first))
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
    fn multi_character_pattern_is_rejected_for_now() {
        assert!(parse("ab").is_err());
    }
}
