//! Worst-case input generation: given a pattern, builds a concrete input
//! string designed to drive the instrumented engine's step count into
//! exponential/polynomial blowup — `reps` copies of a character the
//! pattern's repeated unit matches, followed by one character it doesn't,
//! so a fullmatch attempt is forced to exhaust every backtracking split
//! before failing at the very end.

use crate::parser::{Ast, CharClass};

/// Does *any* leaf in `ast` accept `c`? A structural heuristic (not a full
/// match), just precise enough to pick a representative "matching" char
/// and a distinct "non-matching" char for [`worst_case`].
pub(crate) fn char_is_accepted(ast: &Ast, c: char) -> bool {
    match ast {
        Ast::Empty | Ast::AnchorStart | Ast::AnchorEnd => false,
        Ast::Literal(l) => *l == c,
        Ast::CharClass(class) => class.matches(c),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            nodes.iter().any(|n| char_is_accepted(n, c))
        }
        Ast::Repeat { node, .. } | Ast::Group(node) => char_is_accepted(node, c),
    }
}

/// Finds a character `ast` can plausibly match somewhere, to seed the
/// repeated unit of a worst-case input.
pub(crate) fn representative_char(ast: &Ast) -> Option<char> {
    match ast {
        Ast::Empty | Ast::AnchorStart | Ast::AnchorEnd => None,
        Ast::Literal(c) => Some(*c),
        Ast::CharClass(class) => representative_char_for_class(class),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().find_map(representative_char),
        Ast::Repeat { node, .. } | Ast::Group(node) => representative_char(node),
    }
}

fn representative_char_for_class(class: &CharClass) -> Option<char> {
    if class.negated {
        ('a'..='z').chain('0'..='9').find(|&c| class.matches(c))
    } else {
        class.ranges.first().map(|&(lo, _)| lo)
    }
}

/// A small pool of candidate "shouldn't match" characters to try as the
/// worst-case input's trailing char, roughly in order of how unlikely a
/// typical pattern is to accept them. `\n` is last: it's rejected by every
/// class here except `.` (which excludes it precisely so this works), so it
/// only gets picked for the patterns that actually need it.
const TAIL_CANDIDATES: [char; 11] = ['!', '@', '#', '$', '%', '^', '&', '~', 'Z', '0', '\n'];

fn pick_tail_char(ast: &Ast, unit_char: char) -> char {
    TAIL_CANDIDATES
        .into_iter()
        .find(|&c| c != unit_char && !char_is_accepted(ast, c))
        .unwrap_or(if unit_char == '!' { '?' } else { '!' })
}

/// Generate a worst-case input for `ast`: `reps` copies of a char the
/// pattern matches, plus one trailing char it (heuristically) doesn't —
/// forcing a fullmatch attempt to exhaust backtracking before failing.
pub fn worst_case(ast: &Ast, reps: usize) -> String {
    let unit_char = representative_char(ast).unwrap_or('a');
    let tail_char = pick_tail_char(ast, unit_char);
    let mut input = String::with_capacity(reps + 1);
    for _ in 0..reps {
        input.push(unit_char);
    }
    input.push(tail_char);
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_case_repeats_unit_char_and_appends_a_tail_char() {
        let ast = Ast::Repeat {
            node: Box::new(Ast::Literal('a')),
            min: 1,
            max: None,
        };
        let input = worst_case(&ast, 5);
        assert_eq!(input.len(), 6);
        assert!(input.chars().take(5).all(|c| c == 'a'));
        assert_ne!(input.chars().last(), Some('a'));
    }

    #[test]
    fn worst_case_of_zero_reps_is_just_the_tail_char() {
        let ast = Ast::Literal('a');
        assert_eq!(worst_case(&ast, 0).chars().count(), 1);
    }

    #[test]
    fn tail_char_is_not_accepted_by_the_pattern() {
        let ast = Ast::CharClass(CharClass {
            negated: false,
            ranges: vec![('0', '9')],
        });
        let input = worst_case(&ast, 3);
        let tail = input.chars().last().unwrap();
        assert!(!char_is_accepted(&ast, tail));
    }

    #[test]
    fn tail_char_falls_back_to_newline_for_a_dot_based_pattern() {
        // `.` (negated, excludes only '\n') accepts every other candidate
        // in TAIL_CANDIDATES, so '\n' is the only char left that can force
        // a fullmatch failure — without it, worst-case generation for
        // patterns built on `.` (e.g. `(.+)+`) can't demonstrate blowup.
        let dot = Ast::CharClass(CharClass {
            negated: true,
            ranges: vec![('\n', '\n')],
        });
        let input = worst_case(&dot, 5);
        assert_eq!(input.chars().last(), Some('\n'));
        assert!(!char_is_accepted(&dot, '\n'));
    }

    #[test]
    fn representative_char_picks_a_class_member() {
        let ast = Ast::CharClass(CharClass {
            negated: false,
            ranges: vec![('x', 'z')],
        });
        let c = representative_char(&ast).unwrap();
        assert!(('x'..='z').contains(&c));
    }

    #[test]
    fn representative_char_of_empty_is_none() {
        assert_eq!(representative_char(&Ast::Empty), None);
    }
}
