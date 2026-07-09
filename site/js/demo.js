// The wow moment: type (or one-click-pick) a regex, run it through the
// real instrumented engine, and watch the worst-case input, step counter,
// and risk gauge respond — reproducing the demo generally, not just for
// the preset pattern this page loads with.

import { wasmReady } from "./wasm-loader.js";

const patternInput = document.getElementById("pattern-input");
const runBtn = document.getElementById("run-btn");
const errorEl = document.getElementById("pattern-error");
const worstCaseEl = document.getElementById("worst-case-input");
const stepCounterEl = document.getElementById("step-counter");
const gaugeEl = document.getElementById("risk-gauge");
const gaugeLabelEl = document.getElementById("gauge-label");
const chips = document.querySelectorAll(".chip");
const suggestBtn = document.getElementById("suggest-btn");
const fixPanelEl = document.getElementById("fix-panel");
const fixCompareEl = document.getElementById("fix-compare");
const fixEmptyEl = document.getElementById("fix-empty");
const fixBeforePatternEl = document.getElementById("fix-before-pattern");
const fixBeforeStepsEl = document.getElementById("fix-before-steps");
const fixAfterPatternEl = document.getElementById("fix-after-pattern");
const fixAfterStepsEl = document.getElementById("fix-after-steps");
const fixConfirmEl = document.getElementById("fix-confirm");

// Repetitions of the generated adversarial unit fed into the engine — high
// enough that a catastrophic pattern clears 1,000,000 steps comfortably.
const WORST_CASE_REPS = 24;
const STEP_CEILING = 5_000_000;
const REVEAL_DURATION_MS = 1200;

const GAUGE_LABELS = {
  idle: "Idle",
  running: "Running…",
  safe: "Safe",
  suspicious: "Suspicious",
  catastrophic: "Catastrophic",
};

let running = false;

// The most recently completed run's pattern/worst-case input/step count —
// the "before" side of the suggest-fix comparison (3.2) re-uses this
// instead of re-measuring the original pattern from scratch.
let lastRun = null;

function resetFixPanel() {
  lastRun = null;
  suggestBtn.disabled = true;
  fixPanelEl.hidden = true;
  fixCompareEl.hidden = true;
  fixEmptyEl.hidden = true;
  fixConfirmEl.hidden = true;
}

function setError(message) {
  if (message) {
    errorEl.textContent = message;
    errorEl.hidden = false;
    patternInput.setAttribute("aria-invalid", "true");
  } else {
    errorEl.hidden = true;
    errorEl.textContent = "";
    patternInput.removeAttribute("aria-invalid");
  }
}

function setGauge(risk) {
  gaugeEl.dataset.risk = risk;
  gaugeLabelEl.textContent = GAUGE_LABELS[risk] ?? risk;
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

function easeOutCubic(t) {
  return 1 - Math.pow(1 - t, 3);
}

// Ticks the displayed counter from 0 up to `target` over a fixed duration
// (independent of how fast the real computation was) so the blowup is
// felt rather than instantly reported, per docs/BACKLOG.md 1.1.
function animateCounter(target) {
  return new Promise((resolve) => {
    const startTime = performance.now();
    function tick(now) {
      const t = Math.min((now - startTime) / REVEAL_DURATION_MS, 1);
      const value = Math.round(target * easeOutCubic(t));
      stepCounterEl.textContent = value.toLocaleString("en-US");
      if (t < 1) {
        requestAnimationFrame(tick);
      } else {
        resolve();
      }
    }
    requestAnimationFrame(tick);
  });
}

// Calls run_chunk with a doubling step budget across animation frames
// instead of one huge synchronous call, so a genuinely catastrophic
// pattern never blocks the main thread while its true step count is
// determined (docs/BACKLOG.md 1.4).
async function measureSteps(wasm, pattern, input) {
  let budget = 20_000;
  for (;;) {
    const result = wasm.run_chunk(pattern, input, budget);
    if (!result.truncated || budget >= STEP_CEILING) {
      return result;
    }
    budget = Math.min(budget * 2, STEP_CEILING);
    await nextFrame();
  }
}

async function runDemo(pattern) {
  if (running) {
    return;
  }
  running = true;
  runBtn.disabled = true;
  setError(null);
  setGauge("running");
  stepCounterEl.textContent = "0";
  worstCaseEl.textContent = "…";
  resetFixPanel();

  try {
    const wasm = await wasmReady;

    let worstCase;
    let risk;
    try {
      worstCase = wasm.worst_case_input(pattern, WORST_CASE_REPS);
      risk = wasm.classify_risk(pattern);
    } catch (err) {
      setError(String(err));
      setGauge("idle");
      worstCaseEl.textContent = "—";
      return;
    }

    worstCaseEl.textContent = worstCase;
    const result = await measureSteps(wasm, pattern, worstCase);
    await animateCounter(result.steps_so_far);
    setGauge(risk.toLowerCase());

    if (risk.toLowerCase() !== "safe") {
      lastRun = { pattern, worstCase, steps: result.steps_so_far };
      suggestBtn.disabled = false;
    }
  } catch (err) {
    setError("The engine isn't available in this build yet — run the WASM build.");
    setGauge("idle");
    console.error("ReDoSaur: demo run failed.", err);
  } finally {
    runBtn.disabled = false;
    running = false;
  }
}

runBtn.addEventListener("click", () => runDemo(patternInput.value.trim()));

patternInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    runDemo(patternInput.value.trim());
  }
});

chips.forEach((chip) => {
  chip.addEventListener("click", () => {
    chips.forEach((c) => c.setAttribute("aria-pressed", "false"));
    chip.setAttribute("aria-pressed", "true");
    patternInput.value = chip.dataset.pattern;
    runDemo(chip.dataset.pattern);
  });
});
