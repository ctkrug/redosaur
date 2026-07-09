# ReDoSaur 🦖

**▶ Live demo — [apps.charliekrug.com/redosaur](https://apps.charliekrug.com/redosaur/)**

[![CI](https://github.com/ctkrug/redosaur/actions/workflows/ci.yml/badge.svg)](https://github.com/ctkrug/redosaur/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-39ff6a.svg)](LICENSE)

> Paste a regex. Watch it explode.

ReDoSaur tells you whether a regular expression can hang your server, by actually running it. It
parses your pattern into a real AST, simulates the same backtracking a production engine (PCRE,
Python `re`, JavaScript `RegExp`, Java `Pattern`) would perform, counts every backtracking step,
generates a genuine worst-case input, and shows the exponential blowup as a live counter. Then it
suggests a safe rewrite and re-runs it side by side to prove the fix works.

It is for developers and reviewers who write or approve regexes against untrusted input, form
validators, log parsers, request routers, and want to check one before it ships (or explain to a
skeptical PR reviewer why a regex is a real denial-of-service bug).

## Why it exists

A pattern like `^(a+)+$` compiles cleanly, passes every unit test, and survives code review, then
one 30-character request pins a CPU core for the rest of the day. That is
[ReDoS](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS), a
regularly-exploited denial-of-service class (Cloudflare took a global outage from one in 2019, Stack
Overflow in 2016, and CVEs land against npm and PyPI packages on a steady drip). It stays invisible
because "does this regex work" and "is this regex safe" look identical from the outside and are
completely different questions.

Most tools that catch this pattern-match your regex *source* against a list of known-bad shapes.
That is fast but shallow: it flags safe patterns that merely look dangerous (false positives that
train you to ignore the tool) and misses genuinely dangerous patterns that are not on the list (the
false negatives that actually get you). A CLI that prints `SEVERITY: HIGH` is easy to wave through
in review.

ReDoSaur does not guess from shape. It runs the pattern, measures the real step explosion, and shows
it to you. That turns "trust me, this is dangerous" into "watch this 24-character string take five
million backtracking steps to fail."

## Sample output

Running `(a+)+` against its generated worst-case input:

```
Pattern under test    /(a+)+/
Worst-case input      aaaaaaaaaaaaaaaaaaaaaaaa!
Backtracking steps    5,000,000   (ceiling hit)
Risk                  ● CATASTROPHIC

Suggested fix
  Before  (a+)+   5,000,000 steps
  After   a+              25 steps
  ✓ Fix confirmed: blowup eliminated
```

The web demo animates the counter climbing in real time so the blowup is felt, not just reported.

## How it works

1. **Parse** the regex into an AST: literals, character classes, concatenation, alternation,
   `*` / `+` / `?` / `{m,n}`, capturing and non-capturing groups, and anchors. Malformed input
   returns a positioned parse error instead of a crash.
2. **Simulate** with an instrumented backtracking matcher that walks the AST under `fullmatch`
   semantics, counting every step, exactly the work a real engine does against a non-matching tail.
3. **Detect** structural ambiguity (nested quantifiers, overlapping alternation branches, buried
   variable-length sub-repeats) to *seed* worst-case candidates, then classify Safe / Suspicious /
   Catastrophic from *measured* step growth across increasing input lengths, never from shape alone.
4. **Demonstrate** the worst-case input and the live step counter in the browser.
5. **Suggest a fix**: an equivalent-intent rewrite that removes the ambiguity, re-run against the
   same worst-case input to prove the counter now stops at a handful of steps.

The verdict is trustworthy because it comes from running the engine, not from matching a regex
against a blocklist of other regexes.

## Architecture

- **`crates/core`** (`redosaur-core`): a dependency-free Rust backtracking VM plus the analyzer,
  worst-case generator, and rewrite suggester. Fully unit-tested natively with `cargo test`, with no
  WASM toolchain required. This is where all the logic lives.
- **`crates/wasm`** (`redosaur-wasm`): a thin [`wasm-bindgen`](https://rustwasm.github.io/wasm-bindgen/)
  bridge that exposes the core engine to JavaScript. No logic of its own.
- **`site/`**: a static, dependency-light front end that loads the WASM module directly in the
  browser. No build server, no backend, no account. Your regex never leaves your machine.

## Usage

The tool itself is the [live demo](https://apps.charliekrug.com/redosaur/): paste a pattern (or pick
an example chip), hit **Run**, and watch. Everything below is for hacking on the engine.

```sh
# run the core engine's test suite (no WASM toolchain required)
cargo test -p redosaur-core

# build the WASM bridge (needs the wasm32 target + a matching wasm-bindgen-cli)
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <version from Cargo.lock> --locked
cargo build -p redosaur-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir site/pkg \
  target/wasm32-unknown-unknown/release/redosaur_wasm.wasm

# serve the static site (site/pkg is a build artifact, not checked in)
cd site && python3 -m http.server
```

The core engine's public API, if you want to embed it:

```rust
use redosaur_core::{parser, engine, analyzer, generator, rewrite};

let ast = parser::parse("(a+)+").unwrap();
let input = generator::worst_case(&ast, 24);          // "aaaa…!"
let trace = engine::run(&ast, &input);                // trace.steps, trace.truncated
let risk = analyzer::classify(&ast);                  // Risk::Catastrophic
let fix = rewrite::suggest(&ast);                     // Some("a+")
```

## Design notes

See [`docs/VISION.md`](docs/VISION.md) for the full rationale and [`docs/DESIGN.md`](docs/DESIGN.md)
for the visual direction. In short: a real backtracking engine (not a heuristic) with a hard step
ceiling so the tab never truly hangs; Rust compiled to WASM so a million-step simulation still
animates smoothly; and a pure client-side static site so hosting is trivial and nothing you paste is
ever uploaded.

## License

MIT. See [LICENSE](LICENSE).

---

More of Charlie's projects → [apps.charliekrug.com](https://apps.charliekrug.com)
