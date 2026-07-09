//! Thin `wasm-bindgen` bridge exposing `redosaur-core` to JavaScript. All
//! engine logic lives in `redosaur-core` so it stays unit-testable without
//! a WASM toolchain — this crate only translates types across the JS
//! boundary. Exported functions grow alongside docs/BACKLOG.md.

use redosaur_core::parser;
use wasm_bindgen::prelude::*;

/// Report the core engine's crate version to the UI (used for the footer
/// build tag and bug reports).
#[wasm_bindgen]
pub fn version() -> String {
    redosaur_core::version().to_string()
}

/// Runs once when the WASM module is instantiated.
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

fn parse_error_to_js(err: parser::ParseError) -> JsValue {
    JsValue::from_str(&err.to_string())
}

/// One incremental result of [`run_chunk`]: how many steps the engine had
/// counted when it returned, whether the pattern matched, and whether it
/// hit `budget` before reaching a verdict (meaning the caller should call
/// again with a larger budget to make progress).
#[wasm_bindgen]
pub struct ChunkResult {
    steps_so_far: u32,
    matched: bool,
    truncated: bool,
}

#[wasm_bindgen]
impl ChunkResult {
    #[wasm_bindgen(getter)]
    pub fn steps_so_far(&self) -> u32 {
        self.steps_so_far
    }

    #[wasm_bindgen(getter)]
    pub fn matched(&self) -> bool {
        self.matched
    }

    #[wasm_bindgen(getter)]
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Parses `pattern` and runs the instrumented engine against `input`, up
/// to `budget` steps. Callers drive the live counter by calling this
/// repeatedly across animation frames with an increasing `budget` instead
/// of blocking the main thread on one huge synchronous run.
#[wasm_bindgen]
pub fn run_chunk(pattern: &str, input: &str, budget: u32) -> Result<ChunkResult, JsValue> {
    let ast = parser::parse(pattern).map_err(parse_error_to_js)?;
    let trace = redosaur_core::engine::run_with_ceiling(&ast, input, budget as u64);
    Ok(ChunkResult {
        steps_so_far: trace.steps.min(u32::MAX as u64) as u32,
        matched: trace.matched,
        truncated: trace.truncated,
    })
}

/// Parses `pattern` and generates a worst-case input of `reps` repetitions
/// of its adversarial unit, for the UI to display and feed into
/// [`run_chunk`].
#[wasm_bindgen]
pub fn worst_case_input(pattern: &str, reps: u32) -> Result<String, JsValue> {
    let ast = parser::parse(pattern).map_err(parse_error_to_js)?;
    Ok(redosaur_core::generator::worst_case(&ast, reps as usize))
}

/// Parses `pattern` and classifies its ReDoS risk from measured step
/// growth, returning `"Safe"`, `"Suspicious"`, or `"Catastrophic"`.
#[wasm_bindgen]
pub fn classify_risk(pattern: &str) -> Result<String, JsValue> {
    let ast = parser::parse(pattern).map_err(parse_error_to_js)?;
    let risk = redosaur_core::analyzer::classify(&ast);
    Ok(match risk {
        redosaur_core::analyzer::Risk::Safe => "Safe",
        redosaur_core::analyzer::Risk::Suspicious => "Suspicious",
        redosaur_core::analyzer::Risk::Catastrophic => "Catastrophic",
    }
    .to_string())
}

/// Parses `pattern` and returns a safer, equivalent-intent rewrite if one
/// of the known rules applies, or `undefined` if detection succeeded but
/// no automated fix is known yet for this shape.
#[wasm_bindgen]
pub fn suggest_rewrite(pattern: &str) -> Result<Option<String>, JsValue> {
    let ast = parser::parse(pattern).map_err(parse_error_to_js)?;
    Ok(redosaur_core::rewrite::suggest(&ast, pattern))
}
