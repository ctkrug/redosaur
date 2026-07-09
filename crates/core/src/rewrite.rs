//! Safe-rewrite suggestions: proposes an equivalent-intent pattern that
//! removes the structural ambiguity responsible for catastrophic
//! backtracking. Lands in docs/BACKLOG.md Epic 3.

use crate::parser::Ast;

/// Suggest a safer rewrite of `pattern`, if one is known.
///
/// `None` means no suggestion is available yet — the rewrite rules land in
/// docs/BACKLOG.md Epic 3.
pub fn suggest(_ast: &Ast, _pattern: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_returns_no_suggestion() {
        assert_eq!(suggest(&Ast::Empty, ""), None);
    }
}
