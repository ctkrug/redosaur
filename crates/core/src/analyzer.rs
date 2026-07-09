//! ReDoS risk analysis: combines AST ambiguity detection with the
//! instrumented engine's measured step growth to classify risk.

use crate::engine;
use crate::generator;
use crate::parser::Ast;

/// Input lengths (repetitions of the generated worst-case unit) probed
/// when measuring step growth. Three points are enough to distinguish
/// "grows a bit" from "grows explosively" without spending too many steps
/// on patterns that turn out to be safe.
const PROBE_LENGTHS: [usize; 3] = [8, 16, 24];

/// Step ceiling used while probing growth — high enough that a truly
/// catastrophic pattern blows straight through it (an unambiguous signal),
/// but bounded so classification itself can never hang.
const PROBE_CEILING: u64 = 2_000_000;

/// A growth ratio (last probe's steps / first probe's steps) above this
/// is treated as exponential/high-polynomial blowup.
const CATASTROPHIC_GROWTH: f64 = 4.0;
/// A growth ratio above this (but below [`CATASTROPHIC_GROWTH`]) is
/// treated as worth a second look rather than dismissed as safe.
const SUSPICIOUS_GROWTH: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Safe,
    Suspicious,
    Catastrophic,
}

/// Strips wrapping groups down to the node they contain.
pub(crate) fn peel_groups(ast: &Ast) -> &Ast {
    match ast {
        Ast::Group(inner) => peel_groups(inner),
        other => other,
    }
}

/// Do any two branches of an alternation overlap (accept the same input),
/// making the repeated choice among them ambiguous? Structural equality
/// catches the canonical case (`a|a`); it won't catch every semantically
/// overlapping pair (e.g. `a|[a-z]`), but is precise enough to seed real
/// candidates without false-flagging things like `a|b`.
fn branches_overlap(branches: &[Ast]) -> bool {
    for i in 0..branches.len() {
        for j in (i + 1)..branches.len() {
            if branches[i] == branches[j] {
                return true;
            }
        }
    }
    false
}

/// Can `ast` match strings of more than one possible length — does it
/// contain a repeat whose count isn't pinned to an exact number (i.e.
/// `min != max`)? A repeated body with variable length lets the *same*
/// input be split across the outer repeat's iterations in more than one
/// way, which is the same source of ambiguity as a directly-nested repeat
/// even when it's buried inside a `Concat` alongside other elements — e.g.
/// `(\w+\s?)*` (docs/VISION.md's own canonical example): the outer `*`
/// isn't wrapped around a bare repeat, but `\w+` inside it is still
/// variable-length, so a run of word characters can be split across outer
/// iterations exponentially many ways.
fn has_variable_length_repeat(ast: &Ast) -> bool {
    match ast {
        Ast::Repeat { node, min, max } => *max != Some(*min) || has_variable_length_repeat(node),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            nodes.iter().any(has_variable_length_repeat)
        }
        Ast::Group(inner) => has_variable_length_repeat(inner),
        Ast::Empty | Ast::Literal(_) | Ast::CharClass(_) | Ast::AnchorStart | Ast::AnchorEnd => {
            false
        }
    }
}

/// Does `ast` contain a repeat whose body can match the same input in
/// more than one way — the structural shape (nested quantifiers,
/// overlapping alternation under a repeat, or any variable-length
/// sub-repeat under a repeat) behind catastrophic backtracking?
pub fn has_ambiguous_repeat(ast: &Ast) -> bool {
    match ast {
        Ast::Repeat { node, .. } => {
            let inner = peel_groups(node);
            let flagged = matches!(inner, Ast::Repeat { .. })
                || matches!(inner, Ast::Alternation(branches) if branches_overlap(branches))
                || has_variable_length_repeat(node);
            flagged || has_ambiguous_repeat(node)
        }
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().any(has_ambiguous_repeat),
        Ast::Group(inner) => has_ambiguous_repeat(inner),
        Ast::Empty | Ast::Literal(_) | Ast::CharClass(_) | Ast::AnchorStart | Ast::AnchorEnd => {
            false
        }
    }
}

/// Classify the ReDoS risk of `ast`.
///
/// Structural ambiguity (`has_ambiguous_repeat`) seeds *candidates* — it
/// never classifies on its own. The verdict comes from actually running
/// the instrumented engine against generated worst-case inputs at
/// increasing lengths and measuring how steeply the step count grows:
/// that's what makes the result trustworthy rather than pattern-matched
/// (docs/VISION.md).
pub fn classify(ast: &Ast) -> Risk {
    if !has_ambiguous_repeat(ast) {
        return Risk::Safe;
    }

    let mut steps = Vec::with_capacity(PROBE_LENGTHS.len());
    for &len in &PROBE_LENGTHS {
        let input = generator::worst_case(ast, len);
        let trace = engine::run_with_ceiling(ast, &input, PROBE_CEILING);
        if trace.truncated {
            // Hit the ceiling before even reaching a verdict — an
            // unambiguous sign of catastrophic blowup.
            return Risk::Catastrophic;
        }
        steps.push(trace.steps);
    }

    let first = steps.first().copied().unwrap_or(1).max(1);
    let last = steps.last().copied().unwrap_or(1);
    let growth = last as f64 / first as f64;

    if growth > CATASTROPHIC_GROWTH {
        Risk::Catastrophic
    } else if growth > SUSPICIOUS_GROWTH {
        Risk::Suspicious
    } else {
        Risk::Safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn empty_ast_is_safe() {
        assert_eq!(classify(&Ast::Empty), Risk::Safe);
    }

    #[test]
    fn risk_levels_are_ordered() {
        assert!(Risk::Safe < Risk::Suspicious);
        assert!(Risk::Suspicious < Risk::Catastrophic);
    }

    #[test]
    fn nested_plus_of_plus_is_flagged() {
        assert!(has_ambiguous_repeat(&parse("(a+)+").unwrap()));
    }

    #[test]
    fn nested_star_of_star_is_flagged() {
        assert!(has_ambiguous_repeat(&parse("(a*)*").unwrap()));
    }

    #[test]
    fn nested_plus_of_star_is_flagged() {
        assert!(has_ambiguous_repeat(&parse("(a+)*").unwrap()));
    }

    #[test]
    fn overlapping_alternation_under_star_is_flagged() {
        assert!(has_ambiguous_repeat(&parse("(a|a)*").unwrap()));
    }

    #[test]
    fn plain_plus_is_not_flagged() {
        assert!(!has_ambiguous_repeat(&parse("a+").unwrap()));
    }

    #[test]
    fn char_class_plus_is_not_flagged() {
        assert!(!has_ambiguous_repeat(&parse("[a-z]+").unwrap()));
    }

    #[test]
    fn grouped_concat_plus_is_not_flagged() {
        assert!(!has_ambiguous_repeat(&parse("(ab)+").unwrap()));
    }

    #[test]
    fn non_overlapping_alternation_under_repeat_is_not_flagged() {
        // (a|b)+ — distinct, non-overlapping branches under a repeat is
        // exactly the shape branches_overlap must clear: two structurally
        // different alternatives can't be re-split against each other the
        // way (a|a)+'s identical branches can.
        assert!(!has_ambiguous_repeat(&parse("(a|b)+").unwrap()));
    }

    #[test]
    fn nested_quantifier_through_non_capturing_group_is_flagged() {
        // peel_groups must see through (?:...) exactly like (...) — the
        // parser normalizes both to the same Group node (parser.rs).
        assert!(has_ambiguous_repeat(&parse("(?:a+)+").unwrap()));
    }

    #[test]
    fn non_capturing_grouped_concat_plus_is_not_flagged() {
        assert!(!has_ambiguous_repeat(&parse("(?:ab)+").unwrap()));
    }

    #[test]
    fn variable_length_repeat_buried_in_a_concat_is_flagged() {
        // (\w+\s?)* — docs/VISION.md's own canonical example. The outer *
        // isn't wrapped directly around a repeat or an alternation; \w+ is
        // nested inside a two-element Concat instead. Before
        // has_variable_length_repeat, this was a false negative: a
        // genuinely catastrophic pattern the pre-filter never flagged.
        assert!(has_ambiguous_repeat(&parse(r"(\w+\s?)*").unwrap()));
    }

    #[test]
    fn fixed_length_repeat_buried_in_a_concat_is_not_flagged() {
        // Every sub-repeat has min == max (fixed length), so the body
        // can only ever match one length — no split ambiguity.
        assert!(!has_ambiguous_repeat(&parse(r"(a{2}b{3})+").unwrap()));
    }

    #[test]
    fn pathological_patterns_classify_catastrophic() {
        for pattern in ["(a+)+", "(a*)*", "(a+)*", "(a|a)*"] {
            let ast = parse(pattern).unwrap();
            assert_eq!(
                classify(&ast),
                Risk::Catastrophic,
                "expected {pattern} to classify Catastrophic"
            );
        }
    }

    #[test]
    fn safe_patterns_classify_safe() {
        for pattern in ["a+", "[a-z]+", "(ab)+"] {
            let ast = parse(pattern).unwrap();
            assert_eq!(
                classify(&ast),
                Risk::Safe,
                "expected {pattern} to classify Safe"
            );
        }
    }

    #[test]
    fn bounded_repetition_never_classifies_catastrophic() {
        let ast = parse("a{1,20}").unwrap();
        assert_ne!(classify(&ast), Risk::Catastrophic);
    }

    #[test]
    fn moderate_growth_classifies_suspicious_without_truncating() {
        // (a?){20}a{20} trips the structural pre-filter (a bounded a? nested
        // in a bounded outer repeat still looks like a nested repeat), but
        // its measured step growth across probes stays moderate (~2.7x) and
        // never comes close to the 2,000,000-step probe ceiling. Every other
        // Catastrophic-classifying test in this file hits the ceiling and
        // returns early, so without this test the growth-ratio comparisons
        // in `classify` (as opposed to the ceiling-truncation shortcut) had
        // no coverage at all — a mutation to either threshold comparison
        // survived the full suite undetected.
        let ast = parse("(a?){20}a{20}").unwrap();
        assert_eq!(classify(&ast), Risk::Suspicious);
    }

    #[test]
    fn high_growth_classifies_catastrophic_without_truncating() {
        // (a{1,2})+ never hits the 2,000,000-step probe ceiling (it tops
        // out under 1.3M at the largest probe) but its measured growth
        // ratio is over 2,000x — a Catastrophic verdict reached purely via
        // the growth-ratio comparison, not the ceiling-truncation
        // shortcut every other Catastrophic test in this file takes.
        let ast = parse("(a{1,2})+").unwrap();
        assert_eq!(classify(&ast), Risk::Catastrophic);
    }

    #[test]
    fn structurally_flagged_but_measured_safe_pattern_classifies_safe() {
        // (a{1,2}){1,2} trips the structural pre-filter — a bounded
        // variable-length repeat nested in another repeat looks exactly
        // like the (a+)+ shape — but both repeats are so tightly bounded
        // that measured growth is flat (1.0x across every probe length).
        // This is the tool's core premise proven in the safe direction:
        // the pre-filter only ever seeds a candidate, and measurement can
        // clear it all the way down to Safe, not just downgrade it to
        // Suspicious.
        let ast = parse("(a{1,2}){1,2}").unwrap();
        assert!(has_ambiguous_repeat(&ast));
        assert_eq!(classify(&ast), Risk::Safe);
    }

    #[test]
    fn dot_based_nested_quantifier_classifies_catastrophic() {
        // (.+)+ is a very common real-world ReDoS shape; it only
        // classifies correctly if the worst-case generator can find a
        // char '.' rejects to force a fullmatch failure (see generator.rs).
        let ast = parse("(.+)+").unwrap();
        assert_eq!(classify(&ast), Risk::Catastrophic);
    }

    #[test]
    fn word_and_optional_space_repeat_classifies_catastrophic() {
        // (\w+\s?)* — docs/VISION.md's canonical "misses genuinely
        // dangerous patterns" example. Without has_variable_length_repeat
        // this classified Safe, because has_ambiguous_repeat never even
        // ran the engine to measure it.
        let ast = parse(r"(\w+\s?)*").unwrap();
        assert_eq!(classify(&ast), Risk::Catastrophic);
    }
}
