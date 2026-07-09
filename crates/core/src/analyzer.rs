//! ReDoS risk analysis: combines AST ambiguity detection with the
//! instrumented engine's measured step growth to classify risk. The
//! detection logic lands in docs/BACKLOG.md Epic 2.

use crate::parser::Ast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Safe,
    Suspicious,
    Catastrophic,
}

/// Classify the ReDoS risk of `ast`.
///
/// Placeholder: always reports [`Risk::Safe`] until the ambiguity and
/// growth-measurement analysis described in docs/BACKLOG.md Epic 2 is
/// implemented.
pub fn classify(_ast: &Ast) -> Risk {
    Risk::Safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ast_is_safe() {
        assert_eq!(classify(&Ast::Empty), Risk::Safe);
    }

    #[test]
    fn risk_levels_are_ordered() {
        assert!(Risk::Safe < Risk::Suspicious);
        assert!(Risk::Suspicious < Risk::Catastrophic);
    }
}
