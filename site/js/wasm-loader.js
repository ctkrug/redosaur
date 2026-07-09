// Single shared boot of the WASM module. Both the engine-status check and
// the demo import this so the module is only instantiated once.

export const wasmReady = (async () => {
  const wasm = await import("../pkg/redosaur_wasm.js");
  await wasm.default();
  return wasm;
})();
