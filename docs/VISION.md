# ReDoSaur — Vision

## The problem

Regular expressions are load-bearing infrastructure that almost nobody audits. A pattern like
`^(a+)+$` or `^(\w+\s?)*$` compiles without a warning, passes every unit test anyone bothers to
write, and behaves exactly like every other regex — right up until a specific input shape sends
the backtracking engine into exponential (or high-degree polynomial) blowup. That single request
then pins a CPU core for seconds, minutes, or forever, and because the server is usually
single-threaded per worker (Node.js, most WSGI setups), one request can take the whole process
down. This is [ReDoS](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS):
a real, repeatedly-exploited denial-of-service class (Cloudflare 2019, Stack Overflow 2016, and a
steady stream of CVEs against npm/PyPI packages), and it is almost invisible during normal
development because "does the regex work" and "is the regex safe" are completely different
questions that look identical from the outside.

The tools that exist to catch this mostly work by **pattern-matching the regex source** against a
list of known-dangerous shapes: nested quantifiers, alternation with overlapping branches, and so
on. That approach is fast but shallow — it flags safe-but-shaped-like-danger patterns (false
positives that train people to ignore the tool) and misses genuinely dangerous patterns that don't
match the known list (false negatives, which is the failure mode that actually matters). None of
them make the danger *legible* to someone who isn't already a regex-internals expert. A CLI that
prints `SEVERITY: HIGH` is easy to shrug off in a PR review.

## Who it's for

Developers and reviewers who write or review regexes regularly — in application code, in API
input validation, in config/log-parsing tools — and want a way to actually *check* a pattern
before it ships, and a way to *explain* the risk to a teammate or a PR reviewer who's skeptical
that a regex could possibly be a security bug. Secondary audience: people learning about ReDoS as
a vulnerability class, who benefit from seeing the exponential blowup happen instead of reading
about it.

## The core idea

Don't guess. **Run it.** ReDoSaur parses a regex into a real AST and executes it through an
instrumented backtracking VM — the same evaluation strategy real backtracking engines (PCRE,
Python `re`, JavaScript `RegExp`, Java `Pattern`) use — counting every backtracking step taken.
That gives two things static analysis can't:

1. **Empirical proof, not a heuristic score.** Risk is confirmed by *measuring* step count growth
   as a generated worst-case input grows longer (does step count roughly double per added
   character? That's exponential; that's real). A pattern is flagged because it demonstrably
   blows up, not because it resembles a pattern that once did.
2. **A demonstration, not a verdict.** The same engine trace renders in the browser as a live,
   ticking step counter racing upward while the input string animates into place — the "wow
   moment" this project is built around. Watching a 20-character string take millions of steps and
   visibly stall the page is convincing in a way a severity label never is.

Once risk is confirmed, ReDoSaur's rewrite suggester proposes an equivalent-intent pattern that
removes the specific structural ambiguity responsible (e.g. flattening a nested quantifier,
anchoring an unbounded repeat, or restructuring overlapping alternation branches) and re-runs the
same instrumented engine against the same worst-case input to *prove* the fix works — the counter
that climbed into the millions now stops at a handful of steps.

## Key design decisions

- **A real backtracking engine, not a heuristic.** The core value proposition depends on this:
  ReDoSaur must actually simulate backtracking (with a hard step ceiling so the browser tab never
  actually hangs) rather than classify regex source text. This is the entire reason the project is
  "impressive" rather than "a regex linter."
- **Rust compiled to WASM, not JavaScript.** Backtracking simulation for a pathological pattern can
  legitimately run into millions of steps within a fraction of a second — this needs to be fast
  enough that the demo animates smoothly rather than freezing the tab outright. WASM also lets the
  same core engine crate be reused (CLI, tests) without a JS rewrite.
- **Core engine has zero WASM dependency.** `redosaur-core` is pure, dependency-light Rust,
  independently unit-testable with `cargo test`. `redosaur-wasm` is a thin bridge. This keeps the
  hard part (the engine) fast to iterate on and easy to test without a browser or a WASM toolchain.
- **A hard step ceiling, always.** The instrumented engine must cap total steps (configurable, but
  bounded) so that "simulate this pathological regex" can never actually hang the browser tab it's
  running in — the demo shows the counter racing toward the ceiling, not an unresponsive page.
- **Static site, no backend.** The whole product is client-side: parse, simulate, and render all
  happen in the browser via WASM. This keeps hosting trivial (flat files on a CDN/subpath) and
  means nobody's regex ever leaves their machine — a meaningful trust property for a security tool.
- **Detection through measurement, not just structure.** Structural ambiguity detection (nested
  quantifiers etc.) is used to *seed* candidate worst-case inputs efficiently, but the actual
  Suspicious/Catastrophic classification comes from measuring real step growth across increasing
  input lengths — this is what makes the tool's verdict trustworthy rather than pattern-matched.

## What "v1 done" looks like

- Paste a regex, hit test: the AST parses a realistic subset of regex syntax (literals, character
  classes, concatenation, alternation, `*`/`+`/`?`/`{m,n}`, groups, anchors).
- The instrumented engine actually executes the pattern against generated inputs and reports a
  real step count, capped by a hard ceiling.
- For a known-pathological pattern (`^(a+)+$` against a crafted string of `a`s + one non-matching
  character), the UI shows a worst-case input and a live counter climbing into the millions while
  the page visibly works to keep up — the wow moment, reachable with zero setup.
- A risk classification (Safe / Suspicious / Catastrophic) is shown, backed by measured step
  growth across at least a few input lengths, not just a static label.
- Clicking "suggest fix" shows a rewritten pattern and re-runs the same worst-case input against
  it side-by-side, visibly stopping at a low step count.
- The whole thing is a static site: open `index.html` (or its hosted equivalent), no server, no
  account, no setup, works entirely offline once loaded.
- The page itself looks and feels like a finished product per `docs/DESIGN.md` — not a bare form.
