//! Instrumented backtracking matcher: walks an [`crate::Ast`] against an
//! input string and counts every backtracking step taken, the same way a
//! real backtracking regex engine (PCRE, Python `re`, JS `RegExp`) would.
//! `run` requires the whole input to match (like `re.fullmatch`) — this is
//! what makes patterns like `(a+)+` blow up against a non-matching tail,
//! the classic ReDoS shape described in docs/VISION.md.

use crate::parser::Ast;

/// Default hard cap on backtracking steps before a run is truncated. Keeps
/// a genuinely catastrophic pattern from ever hanging the caller.
pub const DEFAULT_STEP_CEILING: u64 = 5_000_000;

/// The result of running the instrumented matcher against one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchTrace {
    pub matched: bool,
    pub steps: u64,
    /// `true` if the step ceiling was hit before a verdict was reached.
    pub truncated: bool,
}

struct Counters {
    steps: u64,
    limit: u64,
    truncated: bool,
}

impl Counters {
    /// Counts one step; returns `true` once the ceiling has been hit.
    fn tick(&mut self) -> bool {
        self.steps += 1;
        if self.steps >= self.limit {
            self.truncated = true;
        }
        self.truncated
    }
}

/// Run the instrumented backtracking engine for `ast` against `input`,
/// requiring the whole input to match, up to [`DEFAULT_STEP_CEILING`] steps.
pub fn run(ast: &Ast, input: &str) -> MatchTrace {
    run_with_ceiling(ast, input, DEFAULT_STEP_CEILING)
}

/// Same as [`run`], with a caller-supplied step ceiling.
pub fn run_with_ceiling(ast: &Ast, input: &str, ceiling: u64) -> MatchTrace {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut counters = Counters {
        steps: 0,
        limit: ceiling,
        truncated: false,
    };
    let matched = match_node(ast, &chars, 0, &mut counters, &|pos, _| pos == len);
    MatchTrace {
        matched,
        steps: counters.steps,
        truncated: counters.truncated,
    }
}

/// Matches `ast` at `pos`, calling the continuation `k` with the position
/// reached on success. `k` returning `false` triggers backtracking into any
/// remaining alternative for this node.
fn match_node(
    ast: &Ast,
    input: &[char],
    pos: usize,
    counters: &mut Counters,
    k: &dyn Fn(usize, &mut Counters) -> bool,
) -> bool {
    if counters.tick() {
        return false;
    }
    match ast {
        Ast::Empty => k(pos, counters),
        Ast::Literal(c) => {
            if input.get(pos) == Some(c) {
                k(pos + 1, counters)
            } else {
                false
            }
        }
        _ => false,
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
        assert!(!trace.truncated);
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
