//! Worst-case input generation: given a pattern flagged as risky, builds a
//! concrete input string that drives the instrumented engine's step count
//! into exponential/polynomial blowup. Lands in docs/BACKLOG.md Epic 2.

use crate::parser::Ast;

/// Generate a candidate worst-case input of roughly `target_len` characters
/// for `ast`.
///
/// Placeholder: repeats `'a'`. Real construction (near-miss suffix,
/// ambiguous-repetition seeding) lands in docs/BACKLOG.md Epic 2.
pub fn worst_case(_ast: &Ast, target_len: usize) -> String {
    "a".repeat(target_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_case_has_requested_length() {
        assert_eq!(worst_case(&Ast::Empty, 5).len(), 5);
    }

    #[test]
    fn worst_case_of_zero_length_is_empty() {
        assert_eq!(worst_case(&Ast::Empty, 0), "");
    }
}
