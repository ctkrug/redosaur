//! ReDoS risk analysis: combines AST ambiguity detection with the
//! instrumented engine's measured step growth to classify risk.

use crate::parser::Ast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Safe,
    Suspicious,
    Catastrophic,
}

/// Strips wrapping groups down to the node they contain.
fn peel_groups(ast: &Ast) -> &Ast {
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

/// Does `ast` contain a repeat whose body can match the same input in
/// more than one way — the structural shape (nested quantifiers,
/// overlapping alternation under a repeat) behind catastrophic
/// backtracking?
pub fn has_ambiguous_repeat(ast: &Ast) -> bool {
    match ast {
        Ast::Repeat { node, .. } => {
            let inner = peel_groups(node);
            let flagged = matches!(inner, Ast::Repeat { .. })
                || matches!(inner, Ast::Alternation(branches) if branches_overlap(branches));
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
/// Structural-only for now: [`Risk::Suspicious`] if an ambiguous repeat is
/// present, [`Risk::Safe`] otherwise. Growth measurement (confirming
/// Catastrophic empirically rather than by shape alone) lands in the next
/// commit per docs/BACKLOG.md 2.3.
pub fn classify(ast: &Ast) -> Risk {
    if has_ambiguous_repeat(ast) {
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
}
