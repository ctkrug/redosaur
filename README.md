# ReDoSaur 🦖

**Paste a regex. Watch it explode.**

ReDoSaur simulates the *actual* backtracking regex engine — instruction by instruction — to
reliably detect catastrophic backtracking (ReDoS), generate a real worst-case input string, and
show you the exponential blowup as a live step counter while your browser visibly chugs. Then it
suggests a safe rewrite, and you watch it stop.

Unlike linters that pattern-match known-bad regex shapes (`(a+)+`, `(a|a)*`, ...) and produce a
lot of false positives/negatives, ReDoSaur actually *runs* an instrumented backtracking matcher
against your pattern, counts real backtracking steps, and proves the risk empirically — the same
way the vulnerability would actually trigger in production.

## Why this exists

Regular expressions look innocuous. A pattern like `^(a+)+$` compiles cleanly, passes code
review, and works fine on every test string anyone tries — until an attacker (or a weird piece of
user input) sends a 30-character string that pins your event loop for the next age of the
universe. This is [ReDoS](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS):
one of the most common and most under-detected classes of denial-of-service bugs, precisely
because it's invisible until it isn't.

Most existing tools either:
- **Statically pattern-match** known-dangerous constructs against your regex source — fast, but
  full of false positives (safe patterns flagged) and false negatives (novel dangerous shapes
  missed).
- **Assert risk with a score** and expect you to trust it.

ReDoSaur instead builds a small instrumented backtracking VM, actually executes your pattern
against crafted inputs, counts the real step explosion, and *shows* it to you — turning "trust me,
this is dangerous" into "watch this 20-character string take 4 seconds and 8 million backtracking
steps to fail."

## How it works

1. **Parse** — your regex is parsed into an AST (a real parser: alternation, concatenation,
   repetition, groups, character classes, anchors — not a regex-matches-regex heuristic).
2. **Simulate** — an instrumented Thompson-style backtracking engine walks the AST against a
   candidate input, counting every backtracking step it takes.
3. **Detect** — ReDoSaur searches for structural ambiguity (nested/overlapping quantifiers that
   allow multiple ways to match the same substring) and uses that analysis to seed candidate
   worst-case inputs, then *measures* the actual step growth as input length increases to confirm
   exponential/polynomial blowup rather than just asserting it from shape.
4. **Demonstrate** — the worst-case input and a live, ticking step counter are rendered in the
   browser in real time, so the blowup is *felt*, not just reported.
5. **Suggest a fix** — a rewritten, equivalent-intent pattern that removes the ambiguity (e.g.
   possessive-style flattening of nested quantifiers, anchoring, or atomic-group equivalents) is
   proposed and simulated side-by-side to prove the fix actually stops the blowup.

## Stack

- **Core engine** — Rust (`crates/core`), a dependency-free backtracking regex VM + analyzer +
  worst-case input generator + rewrite suggester. Fully unit-testable natively (`cargo test`),
  independent of any WASM tooling.
- **WASM bridge** — Rust (`crates/wasm`), a thin [`wasm-bindgen`](https://rustwasm.github.io/wasm-bindgen/)
  wrapper exposing the core engine to JavaScript, compiled to `wasm32-unknown-unknown`.
- **Front end** — a static, dependency-light site (`site/`) that loads the WASM module directly in
  the browser: no build server, no backend, deployable as flat files to any static host or CDN
  subpath.

## Status

The core engine, ReDoS analyzer, worst-case generator, rewrite suggester, and WASM bridge are
built and tested; the site's regex tester is wired up end to end for arbitrary patterns (not
just a fixed demo), including the "suggest fix" side-by-side before/after trace and synthesized
SFX. See [`docs/VISION.md`](docs/VISION.md) for the full design and
[`docs/BACKLOG.md`](docs/BACKLOG.md) for the build plan.

## Development

```sh
# run the core engine's test suite (no WASM toolchain required)
cargo test -p redosaur-core

# build the WASM bridge (requires the wasm32-unknown-unknown target + wasm-bindgen-cli
# matching the wasm-bindgen version in Cargo.lock)
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <matching Cargo.lock version>
cargo build -p redosaur-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir site/pkg \
  target/wasm32-unknown-unknown/release/redosaur_wasm.wasm

# serve the static site (site/pkg is a build artifact, not checked in)
cd site && python3 -m http.server
```

## License

MIT — see [LICENSE](LICENSE).
