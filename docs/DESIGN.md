# ReDoSaur — Design

## 1. Aesthetic direction

**ReDoSaur is a terminal monitoring console: a dark CRT-glow control room for watching a regex
engine run in real time.** Think an old oscilloscope or reactor telemetry panel repurposed to
watch backtracking steps climb — a phosphor-green trace on a near-black scope, amber warning as
risk escalates, red alarm at catastrophic blowup, monospace type throughout, a faint scanline
sheen over everything. This is not generic "dark mode dev tool" — the console framing is load-
bearing: the whole product is built around *watching a number climb in real time*, so the UI
should feel like instrumentation, not a settings form.

This direction (terminal/CRT monitoring, phosphor-green-on-black with amber/red alarm accents) is
a deliberate departure from a generic flat dark theme — the glow, scanline texture, and alarm-
escalation color logic are the point, not decoration.

## 2. Tokens

| Token | Value | Use |
|---|---|---|
| `--bg` | `#080a08` | page background |
| `--surface-1` | `#10140f` | panel background |
| `--surface-2` | `#1a2018` | raised panel / input background |
| `--text` | `#d9f5df` | primary text (phosphor-tinted off-white) |
| `--text-muted` | `#7c9483` | secondary text, labels |
| `--accent` | `#39ff6a` | phosphor green — safe state, trace line, primary actions |
| `--accent-support` | `#ff9f1c` | amber — suspicious state, warnings |
| `--danger` | `#ff3b45` | red — catastrophic state, alarm |
| `--success` | `#39ff6a` | reuses accent — "fix confirmed" state |
| `--border` | `#26301f` | hairline borders on panels/inputs |

**Type pairing:** display font **Space Mono** (700, used for the wordmark and large headings —
it has enough personality at large sizes to read as a brand, not just monospace body text) +
UI font **JetBrains Mono** (400/500, everything else — body copy, labels, code, the regex input
itself). Both load from Google Fonts with `monospace` system fallback. A monospace-only pairing is
correct here: the product's whole subject is engine internals and character-by-character
matching, and mixed-width type would undercut that.

**Spacing unit:** 8px scale — 8 / 16 / 24 / 32 / 48 / 64 / 96.

**Corner radius:** small and technical — 4px on panels/cards, 2px on inputs and buttons. Never
fully rounded (no pill buttons) — sharp corners reinforce the instrumentation feel.

**Shadow / glow:** no drop shadows. Depth comes from **phosphor glow** — `box-shadow: 0 0 12px
-2px var(--accent)` (or `--accent-support` / `--danger` depending on state) on active/focused
panels and on the live trace line, plus a very low-opacity animated scanline overlay
(`repeating-linear-gradient`, ~2% opacity, slow vertical drift) across the whole page for CRT
texture.

**Motion:** UI chrome transitions 150ms ease-out (hover/focus/panel state). Engine-feedback
motion is punchier — step-counter digit ticks at 80–120ms, risk-gauge needle snaps in ≤140ms,
scanline drift is continuous and slow (12s loop, disabled under `prefers-reduced-motion`).

## 3. Layout intent

**The hero is the scope panel** — the regex input plus the live trace: worst-case input string,
step counter, and risk gauge, all in one instrumentation panel. It takes the majority of the
viewport (~65%+ on desktop) and sits above the fold with nothing competing for attention.

- **1440×900 desktop:** a top bar (wordmark + risk-level legend + mute toggle) at ~64px, then a
  two-column layout below: left ~65% is the scope panel (regex input at top, worst-case input +
  step counter + risk gauge below it, "suggest fix" CTA), right ~35% is a stacked rail — pattern
  history / example patterns to try, and (once a fix is suggested) the side-by-side before/after
  trace. No dead margins: the scope panel's internal trace visualization fills its panel.
- **390×844 phone:** single column, full-width. Scope panel first (input → worst-case string →
  counter → gauge → CTA), examples rail collapses below it as a horizontally-scrollable chip row.
  Touch targets ≥44px; the regex input gets a full-width monospace textarea, not a cramped
  single-line field.

## 4. Signature detail

The wordmark **"ReDoSaur"** renders with a subtle animated phosphor flicker (a brief opacity/glow
jitter on load and every ~8s, like a CRT warming up) and a small claw-mark glyph — three angular
strokes in `--accent` — cut into the "R", built as inline SVG. The page-wide scanline overlay
(see Tokens → glow) is the secondary signature texture that ties every screen together.

## 5. Juice plan (the live-demo interaction is the product, so it gets full game-feel treatment)

- **Input → response:** typing/pasting a pattern and hitting "run" starts the trace within
  100ms — the worst-case input string types itself onto the panel character-by-character (a
  60–100ms-per-chunk reveal, not instant paste), building anticipation before the counter starts.
- **Step counter tween:** the counter doesn't jump straight to its final value — it accelerates
  upward digit-by-digit, genuinely reflecting simulation progress (real steps as they're counted,
  batched per animation frame), so a catastrophic pattern *visibly* struggles to keep the counter
  rendering smoothly while a safe one finishes instantly. This lag *is* the demo.
- **Risk escalation:** the gauge needle/indicator snaps from Safe (green) → Suspicious (amber) →
  Catastrophic (red) with a brief pulse + a subtle screen-edge glow in the new state's color when
  it crosses a threshold.
- **"Suggest fix" moment:** clicking it slides in a side-by-side trace of the rewritten pattern
  against the same worst-case input — the new counter finishes almost instantly next to the old
  one still climbing, with a green "confirmed" pulse and checkmark when it lands under the safe
  threshold. This is the payoff shot.
- **Synth SFX (WebAudio, oscillator/noise-generated, zero audio files):**
  - *tick* — a very quiet, rate-throttled short square-wave blip as the counter climbs (throttled
    to ~10/sec max so it reads as texture, not noise).
  - *warn* — a short two-tone amber chirp when risk crosses into Suspicious.
  - *alarm* — a low sawtooth pulse when risk crosses into Catastrophic.
  - *confirm* — a clean rising two-note chime when a suggested fix's trace finishes safely.
  - All SFX are subtle in volume, created lazily on first user gesture (autoplay-policy safe),
    guarded for environments without `AudioContext` (tests, some browsers).
  - A mute toggle (speaker icon, top bar) persists its state in `localStorage` and defaults to
    unmuted-but-quiet on first visit.
- **Reduced motion:** `prefers-reduced-motion` disables the scanline drift, wordmark flicker, and
  screen-edge glow pulse, but keeps the counter and gauge functionally updating (no motion is
  hidden as "load a spinner forever").

Every later BUILD/QA run follows this file. Changing it is a deliberate, own-commit decision.
