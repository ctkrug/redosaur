// Synthesized WebAudio SFX per docs/DESIGN.md's juice plan — oscillators
// only, zero audio files. The AudioContext is created lazily on first
// play (after a user gesture, satisfying autoplay policy) and every call
// is guarded for hosts without AudioContext (older browsers, tests).

let ctx = null;
let muted = false;
let lastTickAt = 0;

// Ticks fire once per animation frame while the counter climbs; throttled
// so they read as texture, not noise, at 60fps.
const TICK_MIN_INTERVAL_MS = 100;

export function setMuted(value) {
  muted = value;
}

function getContext() {
  if (muted) {
    return null;
  }
  const AudioContextClass = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextClass) {
    return null;
  }
  if (!ctx) {
    ctx = new AudioContextClass();
  }
  return ctx;
}

function tone(freq, duration, type, peakGain, startOffset = 0) {
  const audioCtx = getContext();
  if (!audioCtx) {
    return;
  }
  const osc = audioCtx.createOscillator();
  const gain = audioCtx.createGain();
  osc.type = type;
  osc.frequency.value = freq;
  osc.connect(gain);
  gain.connect(audioCtx.destination);

  const start = audioCtx.currentTime + startOffset;
  gain.gain.setValueAtTime(peakGain, start);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
  osc.start(start);
  osc.stop(start + duration);
}

/** A quiet square-wave blip as the step counter climbs. */
export function playTick() {
  const now = performance.now();
  if (now - lastTickAt < TICK_MIN_INTERVAL_MS) {
    return;
  }
  lastTickAt = now;
  tone(1200, 0.02, "square", 0.02);
}

/** A two-tone amber chirp when risk crosses into Suspicious. */
export function playWarn() {
  tone(660, 0.08, "square", 0.05);
  tone(880, 0.08, "square", 0.05, 0.09);
}

/** A low sawtooth pulse when risk crosses into Catastrophic. */
export function playAlarm() {
  tone(110, 0.35, "sawtooth", 0.06);
}

/** A clean rising two-note chime when a suggested fix's trace lands safely. */
export function playConfirm() {
  tone(660, 0.1, "sine", 0.05);
  tone(990, 0.14, "sine", 0.05, 0.1);
}
