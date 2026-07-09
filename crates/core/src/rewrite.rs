//! Safe-rewrite suggestions: proposes an equivalent-intent pattern that
//! removes the structural ambiguity responsible for catastrophic
//! backtracking.

use crate::analyzer::peel_groups;
use crate::parser::{self, Ast};

/// Suggest a safer, equivalent-intent rewrite of `ast`, if one of the known
/// rules applies. `None` means detection succeeded but no automated fix is
/// known yet for this shape.
pub fn suggest(ast: &Ast) -> Option<String> {
    flatten_nested_quantifier(ast)
        .or_else(|| dedup_overlapping_alternation(ast))
        .as_ref()
        .map(parser::to_pattern)
}

/// `(X+)+`, `(X*)*`, `(X+)*`, `(X*)+` all describe "one or more X" or "zero
/// or more X" — the outer repeat contributes nothing but ambiguity. Once
/// either repeat's minimum is 0, an outer repetition can be satisfied by an
/// empty inner match, so the flattened minimum collapses to 0; only when
/// both require at least one repetition does the flattened form still
/// require one.
fn flatten_nested_quantifier(ast: &Ast) -> Option<Ast> {
    let Ast::Repeat {
        node,
        min: outer_min,
        max: None,
    } = ast
    else {
        return None;
    };
    let Ast::Repeat {
        node: inner_node,
        min: inner_min,
        max: None,
    } = peel_groups(node)
    else {
        return None;
    };
    let flattened_min = if *outer_min == 0 || *inner_min == 0 {
        0
    } else {
        1
    };
    Some(Ast::Repeat {
        node: inner_node.clone(),
        min: flattened_min,
        max: None,
    })
}

/// `(a|a)*` repeats a choice between structurally identical branches —
/// removing the duplicates leaves the same language (any branch already
/// accepted the same input as its duplicate) with no repeated ambiguity.
fn dedup_overlapping_alternation(ast: &Ast) -> Option<Ast> {
    let Ast::Repeat { node, min, max } = ast else {
        return None;
    };
    let Ast::Alternation(branches) = peel_groups(node) else {
        return None;
    };
    let mut deduped: Vec<Ast> = Vec::new();
    for branch in branches {
        if !deduped.contains(branch) {
            deduped.push(branch.clone());
        }
    }
    if deduped.len() == branches.len() {
        return None;
    }
    let new_node = if deduped.len() == 1 {
        deduped.into_iter().next().unwrap()
    } else {
        Ast::Alternation(deduped)
    };
    Some(Ast::Repeat {
        node: Box::new(new_node),
        min: *min,
        max: *max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{classify, Risk};
    use crate::engine::run;
    use crate::parser::parse;

    fn suggested_ast(pattern: &str) -> Ast {
        let ast = parse(pattern).unwrap();
        let rewritten = suggest(&ast).unwrap_or_else(|| {
            panic!("expected a rewrite suggestion for {pattern}");
        });
        parse(&rewritten).unwrap()
    }

    #[test]
    fn flattens_nested_plus_of_plus() {
        assert_eq!(suggest(&parse("(a+)+").unwrap()), Some("a+".to_string()));
    }

    #[test]
    fn flattens_nested_quantifier_through_non_capturing_group() {
        assert_eq!(suggest(&parse("(?:a+)+").unwrap()), Some("a+".to_string()));
    }

    #[test]
    fn flattens_nested_star_of_star() {
        assert_eq!(suggest(&parse("(a*)*").unwrap()), Some("a*".to_string()));
    }

    #[test]
    fn flattens_plus_of_star_to_star() {
        assert_eq!(suggest(&parse("(a+)*").unwrap()), Some("a*".to_string()));
    }

    #[test]
    fn flattens_star_of_plus_to_star() {
        assert_eq!(suggest(&parse("(a*)+").unwrap()), Some("a*".to_string()));
    }

    #[test]
    fn dedupes_overlapping_alternation() {
        assert_eq!(suggest(&parse("(a|a)*").unwrap()), Some("a*".to_string()));
    }

    #[test]
    fn dedup_keeps_remaining_alternation_when_more_than_one_branch_survives() {
        // (a|a|b)* has three branches; removing the single duplicate still
        // leaves two distinct branches, so the result must stay an
        // Alternation (a|b)* rather than collapsing to a bare repeat like
        // the two-branch (a|a)* case above.
        assert_eq!(
            suggest(&parse("(a|a|b)*").unwrap()),
            Some("(a|b)*".to_string())
        );
    }

    #[test]
    fn no_suggestion_for_safe_patterns() {
        for pattern in ["a+", "[a-z]+", "(ab)+", "a{1,20}"] {
            assert_eq!(suggest(&parse(pattern).unwrap()), None);
        }
    }

    #[test]
    fn no_suggestion_for_empty_pattern() {
        assert_eq!(suggest(&Ast::Empty), None);
    }

    #[test]
    fn no_suggestion_for_non_overlapping_alternation_under_repeat() {
        assert_eq!(suggest(&parse("(a|b)*").unwrap()), None);
    }

    #[test]
    fn rewritten_patterns_classify_safe_or_suspicious() {
        for pattern in ["(a+)+", "(a*)*", "(a+)*", "(a*)+", "(a|a)*"] {
            let ast = suggested_ast(pattern);
            assert_ne!(
                classify(&ast),
                Risk::Catastrophic,
                "expected the rewrite of {pattern} to no longer classify Catastrophic"
            );
        }
    }

    #[test]
    fn rewritten_patterns_preserve_matched_strings() {
        let cases: &[(&str, &[&str])] = &[
            ("(a+)+", &["", "a", "aaa", "aaab"]),
            ("(a*)*", &["", "a", "aaa", "aaab"]),
            ("(a|a)*", &["", "a", "aaa", "b", "aab"]),
        ];
        for (pattern, inputs) in cases {
            let original = parse(pattern).unwrap();
            let rewritten = suggested_ast(pattern);
            for input in *inputs {
                assert_eq!(
                    run(&original, input).matched,
                    run(&rewritten, input).matched,
                    "expected {pattern} and its rewrite to agree on {input:?}"
                );
            }
        }
    }
}
