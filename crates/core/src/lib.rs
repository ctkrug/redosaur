//! Core ReDoS analysis engine: regex parsing, instrumented backtracking
//! simulation, ambiguity detection, worst-case input generation, and
//! safe-rewrite suggestions. Pure Rust, no WASM dependency — testable
//! natively with `cargo test`.

pub mod analyzer;
pub mod engine;
pub mod generator;
pub mod parser;
pub mod rewrite;

pub use parser::Ast;

/// Crate version, exposed so the WASM bridge can report it to the UI.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
