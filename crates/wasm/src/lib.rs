//! Thin `wasm-bindgen` bridge exposing `redosaur-core` to JavaScript. All
//! engine logic lives in `redosaur-core` so it stays unit-testable without
//! a WASM toolchain — this crate only translates types across the JS
//! boundary. Exported functions grow alongside docs/BACKLOG.md.

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
