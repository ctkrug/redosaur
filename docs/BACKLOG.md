# ReDoSaur — Backlog

Epics and stories for the build. Every story lists concrete, verifiable acceptance criteria —
build implements to them, QA attacks them. See `docs/VISION.md` for the why and
`docs/DESIGN.md` for the visual direction every UI story must follow.

**Story 1.1 is the wow moment** and must be reachable by the end of Epic 1 — the demo lands
before anything optional gets built.

## Epic 1 — Core Engine & Wow-Moment Demo

- [x] **1.1 — Live catastrophic-backtracking demo for a canonical pathological pattern (WOW MOMENT)**
  - A one-click preset (e.g. "try `(a+)+`") on first load runs the instrumented engine against a
    crafted worst-case input with zero typing required.
  - The step counter displayed climbs past 1,000,000 for this preset, and a core-crate test
    asserts the engine's actual counted steps exceed 1,000,000 for this pattern + input.
  - The counter visibly animates over at least ~1 second (not an instant jump to the final
    number) so the blowup is felt, not just reported.

- [x] **1.2 — Full regex grammar in the parser**
  - `parser::parse` supports literals, concatenation, alternation (`|`), quantifiers (`*`, `+`,
    `?`, `{m,n}`), groups `()`, character classes `[...]`, and anchors (`^`, `$`).
  - Unit tests cover at least one pattern per construct listed above, all passing.
  - Malformed input (unbalanced parens, dangling quantifier) returns a `ParseError` with a
    position — never panics; at least 2 malformed-pattern tests confirm this.

- [x] **1.3 — Full instrumented backtracking engine**
  - `engine::run` correctly matches/rejects against Concat, Alternation, Repeat, and Group nodes
    (not just Empty/Literal), verified by representative test strings per construct.
  - A configurable step ceiling (default 5,000,000) halts execution and returns
    `MatchTrace { truncated: true, .. }` instead of hanging; a test confirms the halt fires at
    the configured cap.
  - A test pattern with 3 nested quantifiers against a crafted worst-case input exceeds 100,000
    counted steps, confirming real backtracking (not a shortcut) is happening.

- [x] **1.4 — WASM bridge exposes chunked parse+run to JS**
  - `redosaur-wasm` exposes a function that parses a pattern and runs the engine against a given
    input up to a per-call step budget, returning `{ steps_so_far, matched, truncated }` so JS
    can call it repeatedly across animation frames instead of blocking the main thread.
  - A test (wasm-bindgen-test, or a documented manual browser check recorded in the PR) confirms
    a single call with a 50,000-step budget returns in under ~16ms, keeping the UI thread
    responsive.

## Epic 2 — ReDoS Detection & Risk Classification

- [x] **2.1 — Structural ambiguity detector seeds candidate risk patterns**
  - `analyzer` inspects the AST for nested/overlapping quantifiers (e.g. `(a+)+`, `(a|a)*`,
    `(a*)*`) and flags them as measurement candidates.
  - Unit tests confirm at least 4 known-pathological shapes are flagged as candidates, and at
    least 3 known-safe shapes (e.g. `a+`, `[a-z]+`, `(ab)+`) are not.

- [x] **2.2 — Worst-case input generator produces genuinely adversarial strings**
  - `generator::worst_case` builds an input from a flagged-ambiguous AST designed to maximize
    backtracking (e.g. `n` repetitions of the ambiguous unit plus one non-matching trailing
    character), replacing the current fixed-placeholder implementation.
  - For each of the 4 pathological patterns from 2.1, running the generated input through the
    engine at increasing `n` shows measurably superlinear growth (step count at n=20 is more
    than 4x the step count at n=10).

- [x] **2.3 — Growth measurement confirms risk empirically**
  - `analyzer::classify` runs the engine against generated worst-case inputs at 2–3 increasing
    lengths and classifies Safe/Suspicious/Catastrophic from measured step growth, not AST shape
    alone.
  - The 4 pathological patterns from 2.1 classify Catastrophic; the 3 safe patterns classify
    Safe; a bounded-repetition case (e.g. `a{1,20}`) classifies Safe or Suspicious, never
    Catastrophic.

- [x] **2.4 — Risk gauge and live trace UI**
  - The site renders a Safe/Suspicious/Catastrophic gauge driven by the WASM analyzer's live
    classification, using `docs/DESIGN.md`'s green/amber/red tokens and escalation pulse.
  - Pasting any of the 4 canonical pathological patterns and hitting "run" shows the worst-case
    input, a live-climbing counter, and the gauge landing on Catastrophic for arbitrary (not
    just preset) input — reproducing the wow moment generally.

## Epic 3 — Safe Rewrite & Fix Confirmation

- [ ] **3.1 — Rewrite rules for common ambiguous shapes**
  - `rewrite::suggest` returns an equivalent-intent rewritten pattern for at least the
    nested-quantifier shape (e.g. `(a+)+` → a flattened `a+`) and the overlapping-alternation
    shape (e.g. `(a|a)*` → a deduplicated alternation).
  - Tests confirm the rewritten pattern still matches every string a representative test set says
    the original matches, and that it classifies Safe or Suspicious (never Catastrophic) via
    `analyzer::classify`.

- [ ] **3.2 — Side-by-side before/after confirmation UI**
  - Clicking "suggest fix" runs the original and rewritten pattern's traces side-by-side against
    the same worst-case input, per `docs/DESIGN.md`'s juice plan (fast-finishing rewritten trace,
    "confirmed" pulse + chime).
  - For both rules from 3.1, the UI demonstrably shows the rewritten trace completing in under
    1,000 steps against an input where the original exceeded 100,000.

- [ ] **3.3 — Graceful "no rewrite available" state**
  - When `rewrite::suggest` returns `None`, the UI shows a designed empty state (not blank, not
    an error) explaining detection succeeded but no automated fix exists yet.
  - QA confirms pasting a Catastrophic pattern outside the two known rewrite rules shows this
    empty state rather than a broken or missing button.

## Epic 4 — Polish, Design & Ship

- [ ] **4.1 — Design polish pass across breakpoints**
  - The site composes with no horizontal scroll, no overlap, and no dead empty margins at 390px,
    768px, and 1440px, per `docs/DESIGN.md`'s layout intent.
  - Every interactive control (regex input, buttons, mute toggle, example chips) has themed
    hover/focus/active/disabled states — no naked native widgets.

- [ ] **4.2 — Brand assets and accessibility pass**
  - A generated favicon (accent green + claw glyph, inline SVG data URI) replaces any default
    icon; the wordmark uses the Space Mono flicker treatment from `docs/DESIGN.md`.
  - Body text contrast is ≥4.5:1 against its background; icon-only buttons carry `aria-label`s;
    the step counter/gauge use a live region so risk escalation is announced; `prefers-reduced-
    motion` disables scanline/flicker/glow-pulse animation while keeping function.

- [ ] **4.3 — Static build pipeline and subpath deploy readiness**
  - `site/` builds to a single self-contained output directory using only relative asset paths
    (no leading `/`).
  - Serving the built output from a non-root subpath (e.g. `python3 -m http.server` under a
    `/redosaur/` prefix) loads the page with zero broken asset requests in the browser devtools
    network tab.
  - CI builds the WASM bundle and static site on every push as a check.
