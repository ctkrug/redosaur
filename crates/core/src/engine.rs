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
        Ast::CharClass(class) => match input.get(pos) {
            Some(&c) if class.matches(c) => k(pos + 1, counters),
            _ => false,
        },
        Ast::AnchorStart => {
            if pos == 0 {
                k(pos, counters)
            } else {
                false
            }
        }
        Ast::AnchorEnd => {
            if pos == input.len() {
                k(pos, counters)
            } else {
                false
            }
        }
        Ast::Concat(nodes) => match_concat(nodes, input, pos, counters, k),
        Ast::Group(inner) => match_node(inner, input, pos, counters, k),
        Ast::Alternation(branches) => {
            for branch in branches {
                if match_node(branch, input, pos, counters, k) {
                    return true;
                }
                if counters.truncated {
                    return false;
                }
            }
            false
        }
        _ => false,
    }
}

/// Matches a sequence of nodes by chaining each node's continuation into
/// matching the rest of the sequence, then finally into `k`.
fn match_concat(
    nodes: &[Ast],
    input: &[char],
    pos: usize,
    counters: &mut Counters,
    k: &dyn Fn(usize, &mut Counters) -> bool,
) -> bool {
    match nodes.split_first() {
        None => k(pos, counters),
        Some((first, rest)) => {
            match_node(first, input, pos, counters, &|p, c| {
                match_concat(rest, input, p, c, k)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CharClass;

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

    #[test]
    fn char_class_matches_member() {
        let digits = Ast::CharClass(CharClass {
            negated: false,
            ranges: vec![('0', '9')],
        });
        assert!(run(&digits, "5").matched);
        assert!(!run(&digits, "x").matched);
    }

    #[test]
    fn anchor_start_only_matches_at_position_zero() {
        // Concat([AnchorStart, Literal('a')]) — anchor must hold before 'a'.
        let ast = Ast::Concat(vec![Ast::AnchorStart, Ast::Literal('a')]);
        assert!(run(&ast, "a").matched);
    }

    #[test]
    fn anchor_end_requires_end_of_input() {
        let ast = Ast::Concat(vec![Ast::Literal('a'), Ast::AnchorEnd]);
        assert!(run(&ast, "a").matched);
    }

    #[test]
    fn concat_matches_each_node_in_sequence() {
        let ast = Ast::Concat(vec![Ast::Literal('a'), Ast::Literal('b')]);
        assert!(run(&ast, "ab").matched);
        assert!(!run(&ast, "ba").matched);
        assert!(!run(&ast, "a").matched);
    }

    #[test]
    fn group_matches_its_inner_node() {
        let ast = Ast::Group(Box::new(Ast::Literal('a')));
        assert!(run(&ast, "a").matched);
    }

    #[test]
    fn alternation_matches_any_branch() {
        let ast = Ast::Alternation(vec![Ast::Literal('a'), Ast::Literal('b')]);
        assert!(run(&ast, "a").matched);
        assert!(run(&ast, "b").matched);
        assert!(!run(&ast, "c").matched);
    }

    #[test]
    fn alternation_backtracks_into_later_branches() {
        // First branch ("a") matches a prefix but leaves "b" unconsumed
        // under fullmatch semantics, so the engine must fall through to
        // the second branch ("ab") to succeed.
        let ast = Ast::Alternation(vec![
            Ast::Literal('a'),
            Ast::Concat(vec![Ast::Literal('a'), Ast::Literal('b')]),
        ]);
        assert!(run(&ast, "ab").matched);
    }
}
