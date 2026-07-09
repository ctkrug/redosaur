# ReDoSaur — Architecture

A concise map of the codebase for anyone (including a future run of this build) picking the
project back up. See `docs/VISION.md` for why, `docs/DESIGN.md` for the visual direction, and
`docs/BACKLOG.md` for what's done vs. outstanding.

## Layout

```
crates/
  core/            # redosaur-core: pure Rust, zero WASM dependency, cargo-testable
    src/parser.rs      # regex -> Ast (recursive-descent)
    src/engine.rs       # instrumented backtracking matcher (Ast, input) -> MatchTrace
    src/analyzer.rs     # Ast -> Risk (structural ambiguity + measured step growth)
    src/generator.rs    # Ast -> adversarial worst-case input string
    src/rewrite.rs      # Ast -> Option<safer pattern> (placeholder — Epic 3)
    src/lib.rs          # re-exports + crate version()
  wasm/            # redosaur-wasm: thin wasm-bindgen bridge, compiled to wasm32-unknown-unknown
    src/lib.rs          # run_chunk / worst_case_input / classify_risk / version
site/
  index.html       # single static page: topbar, scope panel (pattern input + trace), examples rail
  css/style.css    # terminal/CRT design tokens + component styles (docs/DESIGN.md)
  js/wasm-loader.js  # boots the WASM module once behind a shared promise
  js/main.js         # engine-status footer line + mute toggle (localStorage)
  js/demo.js         # the regex tester: input/chips -> WASM calls -> counter + gauge
docs/              # VISION, DESIGN, BACKLOG, ARCHITECTURE (this file)
```

## Data flow (the wow moment)

1. User types a pattern (or clicks an example chip) and hits Run / Enter.
2. `demo.js` calls into the WASM bridge:
   - `worst_case_input(pattern, reps)` — parses the pattern and generates an adversarial input
     (`generator::worst_case`): `reps` copies of a character the pattern matches, plus one
     trailing character it doesn't, forcing a fullmatch attempt to exhaust backtracking.
   - `classify_risk(pattern)` — parses and classifies Safe/Suspicious/Catastrophic
     (`analyzer::classify`) from measured step growth across increasing input lengths, not AST
     shape alone.
   - `run_chunk(pattern, input, budget)` — parses and runs the instrumented engine
     (`engine::run_with_ceiling`) up to `budget` steps. `demo.js`'s `measureSteps()` calls this
     repeatedly with a doubling budget across animation frames so a genuinely catastrophic
     pattern never blocks the main thread while its true step count is determined.
3. `demo.js`'s `animateCounter()` reveals the real step count with a fixed ~1.2s ease-out
   count-up (independent of how fast the underlying computation actually was), and the risk
   gauge lands on its final Safe/Suspicious/Catastrophic state.

## The engine (`engine.rs`)

`run`/`run_with_ceiling` require the **whole input to match** (like `re.fullmatch`), which is
what makes a pattern like `(a+)+` blow up against a non-matching tail — a fullmatch attempt must
exhaust every way of splitting the run of `a`s across outer/inner repetitions before concluding
failure. The matcher is continuation-passing (`match_node(ast, input, pos, counters, k)`): each
node tries to match at `pos` and calls `k` with the position reached; `k` returning `false`
triggers backtracking into any remaining alternative (another repetition, another alternation
branch). `Counters` tracks the step count and a hard ceiling (`DEFAULT_STEP_CEILING =
5_000_000`) so a truncated run reports `MatchTrace { truncated: true, .. }` instead of hanging.

## The analyzer (`analyzer.rs`)

`has_ambiguous_repeat` is a structural pre-filter (nested quantifiers like `(a+)+`/`(a*)*`, or a
repeat over an alternation with structurally-equal branches like `(a|a)*`) that seeds
*candidates* — it never classifies on its own. `classify` only runs the engine (against
`generator::worst_case` inputs at 3 increasing lengths) when the pre-filter flags something,
and the verdict comes from the measured growth ratio between the shortest and longest probe,
not from the shape alone.

## Running things

- Core tests: `cargo test -p redosaur-core` (also runs under `cargo test --workspace`).
- Lint/format (core only — see rationale below): `cargo fmt -p redosaur-core -- --check`,
  `cargo clippy -p redosaur-core --all-targets -- -D warnings`.
- WASM build: `cargo build -p redosaur-wasm --target wasm32-unknown-unknown --release`, then
  `wasm-bindgen --target web --out-dir site/pkg <target-dir>/redosaur_wasm.wasm` to generate the
  `site/pkg/` glue `site/js/wasm-loader.js` imports (gitignored — a build step, not checked in).
- Serve the site locally: `python3 -m http.server` from `site/` (or any static file server) —
  it's a flat, dependency-free static page.

Note: `redosaur-wasm` has no native `#[cfg(test)]` unit tests — `wasm-bindgen`'s `JsValue` calls
abort outside a real JS host, so native `cargo test` on that crate isn't viable. Its logic is
thin translation over `redosaur-core` (which is fully covered natively); the bridge itself is
verified by building for `wasm32-unknown-unknown` (CI does this on every push) plus a manual
check running the compiled module in a JS engine.
