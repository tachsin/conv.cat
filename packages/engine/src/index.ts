// @conv.cat/engine — the seam between crates/conv-core (via WASM or native Tauri bindings) and
// the two apps. See docs/ARCHITECTURE.md "One engine, two runtimes" for the full picture.
//
// The web app and the desktop app import ONLY from this file (or the type-only exports below) —
// never from ./wasm/client.js or ./tauri/client.js directly. Reaching into a specific backend
// from app code defeats the reason this package exists: neither app should know, or need to
// know, which one it's actually talking to.

import { detectBackend } from './backend.js';
import type { ConvEngine, ConvertOptions } from './types.js';

export type { ConvEngine, ConvertOptions, ConvertErrorKind, EngineBackendKind, FormatInfo } from './types.js';
export { ConvertError } from './errors.js';
export { detectBackend } from './backend.js';
export { convertUnit } from './units.js';

let enginePromise: Promise<ConvEngine> | undefined;

/**
 * Returns the process-wide {@link ConvEngine}, picking a backend the first time it's called
 * (Tauri inside a Tauri webview, WASM everywhere else — see {@link detectBackend}) and reusing
 * the same instance after that. This is the one thing most callers need; reach for the instance
 * directly (rather than the {@link convert} convenience export below) only when you also want
 * {@link ConvEngine.listFormats} or {@link ConvEngine.getMemoryCeilingBytes}.
 *
 * `async`, deliberately, even though picking a backend itself is instant: the *import* of
 * `./tauri/client.js` (and its `@tauri-apps/api` dependency) or `./wasm/client.js` has to be
 * dynamic, not a static top-of-file import, or a bundler has no signal to split the backend you
 * won't use into its own chunk — a browser build would otherwise ship `@tauri-apps/api` to every
 * visitor, and a desktop build would ship the WASM client code, neither of which that build ever
 * runs. Confirmed the hard way: this package's own demo harness
 * (`packages/engine/demo/README.md`), which has no bundler to paper over a static import,
 * failed outright on `@tauri-apps/api`'s bare specifiers before this was switched to `import()`.
 */
export function getEngine(): Promise<ConvEngine> {
  enginePromise ??= loadEngine();
  return enginePromise;
}

async function loadEngine(): Promise<ConvEngine> {
  if (detectBackend() === 'tauri') {
    const { TauriBackend } = await import('./tauri/client.js');
    return new TauriBackend();
  }
  const { WasmBackend } = await import('./wasm/client.js');
  return new WasmBackend();
}

/** Convenience wrapper around `(await getEngine()).convert(...)` — see {@link ConvEngine.convert}. */
export async function convert(
  input: Uint8Array,
  from: string,
  to: string,
  options?: ConvertOptions,
): Promise<Uint8Array> {
  const engine = await getEngine();
  return engine.convert(input, from, to, options);
}
