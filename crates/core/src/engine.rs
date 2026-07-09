//! Instrumented backtracking matcher: walks an [`crate::Ast`] against an
//! input string and counts every backtracking step taken, the same way a
//! real backtracking regex engine (PCRE, Python `re`, JS `RegExp`) would.
//! The full simulation lands in docs/BACKLOG.md Epic 1.

use crate::parser::Ast;

/// The result of running the instrumented matcher against one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchTrace {
    pub matched: bool,
    pub steps: u64,
}

/// Run the instrumented backtracking engine for `ast` against `input`,
/// counting backtracking steps as it goes.
pub fn run(ast: &Ast, input: &str) -> MatchTrace {
    match ast {
        Ast::Empty => MatchTrace {
            matched: input.is_empty(),
            steps: 1,
        },
        Ast::Literal(c) => {
            let mut chars = input.chars();
            let matched = chars.next() == Some(*c) && chars.next().is_none();
            MatchTrace { matched, steps: 1 }
        }
        _ => MatchTrace {
            matched: false,
            steps: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ast_matches_empty_input() {
        let trace = run(&Ast::Empty, "");
        assert!(trace.matched);
        assert_eq!(trace.steps, 1);
    }

    #[test]
    fn empty_ast_rejects_nonempty_input() {
        assert!(!run(&Ast::Empty, "x").matched);
    }

    #[test]
    fn literal_matches_exact_char() {
        assert!(run(&Ast::Literal('a'), "a").matched);
    }

    #[test]
    fn literal_rejects_other_char() {
        assert!(!run(&Ast::Literal('a'), "b").matched);
    }
}
