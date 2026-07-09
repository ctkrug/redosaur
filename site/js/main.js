// Boots the WASM engine and wires up the top-bar mute toggle. The engine
// demo itself (regex input, live trace, risk gauge) lands with Epic 1 in
// docs/BACKLOG.md — this file only proves the WASM pipeline loads end to
// end and keeps the mute preference the juice plan (docs/DESIGN.md) needs.

const statusValue = document.getElementById("engine-status-value");
const statusLine = document.getElementById("engine-status");

async function bootEngine() {
  try {
    const wasm = await import("./pkg/redosaur_wasm.js");
    await wasm.default();
    const version = wasm.version();
    statusValue.textContent = `online — v${version}`;
    statusLine.classList.remove("status-line--pending");
  } catch (err) {
    statusValue.textContent = "not built yet (run the WASM build)";
    statusLine.classList.add("status-line--pending");
    console.info("ReDoSaur: WASM module not available in this build.", err);
  }
}

const MUTE_KEY = "redosaur:muted";
const muteToggle = document.getElementById("mute-toggle");

function applyMuteState(muted) {
  muteToggle.setAttribute("aria-pressed", String(muted));
  muteToggle.setAttribute("aria-label", muted ? "Unmute sound effects" : "Mute sound effects");
}

function initMuteToggle() {
  const stored = window.localStorage.getItem(MUTE_KEY);
  let muted = stored === "true";
  applyMuteState(muted);

  muteToggle.addEventListener("click", () => {
    muted = !muted;
    window.localStorage.setItem(MUTE_KEY, String(muted));
    applyMuteState(muted);
  });
}

bootEngine();
initMuteToggle();
