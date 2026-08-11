import type { EngineBackendKind } from './types.js';

/**
 * Picks which {@link EngineBackendKind} this process should use. Tauri's webview injects a
 * global on `window` before any app code runs; everywhere else (a plain browser tab) falls back
 * to the WASM backend. Checks both the v2 (`__TAURI_INTERNALS__`) and v1 (`__TAURI__`) markers —
 * the real desktop app (a separate, not-yet-built backlog ticket — see this package's README)
 * hasn't pinned a major Tauri version in this repo yet, and detecting either keeps this function
 * correct regardless of which one it lands on.
 */
export function detectBackend(): EngineBackendKind {
  if (typeof window !== 'undefined') {
    const injected = window as unknown as {
      __TAURI_INTERNALS__?: unknown;
      __TAURI__?: unknown;
    };
    if (injected.__TAURI_INTERNALS__ !== undefined || injected.__TAURI__ !== undefined) {
      return 'tauri';
    }
  }
  return 'wasm';
}
