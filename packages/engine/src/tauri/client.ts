// The backend `getEngine()` hands back inside a Tauri webview. Calls straight into Tauri's
// stable JS API (`@tauri-apps/api`) — no WASM, no Worker, the conversion runs natively in the
// Rust host process.
//
// IMPORTANT — read before assuming this "just works": the desktop app is still a scaffold (see
// its README and the backlog ticket "real cross-platform Tauri app on the native core"). Nothing
// on the Rust side implements the `convert`/`cancel_conversion`/`list_formats` commands or the
// `conv-progress:{jobId}` event this file calls. This module is written and typechecked against
// that contract so the desktop ticket has a fixed, documented target to build to — see
// packages/engine/README.md "The Tauri contract" for the authoritative version, which that
// ticket must keep in sync with whatever it actually implements.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { fromBackendError } from '../errors.js';
import type { ConvEngine, ConvertOptions, FormatInfo } from '../types.js';

let nextJobId = 1;

export class TauriBackend implements ConvEngine {
  readonly backend = 'tauri' as const;

  async convert(
    input: Uint8Array,
    from: string,
    to: string,
    options: ConvertOptions = {},
  ): Promise<Uint8Array> {
    if (options.signal?.aborted) {
      throw fromBackendError({ kind: 'cancelled', message: 'cancelled before it started' });
    }

    const jobId = nextJobId++;
    let unlistenProgress: UnlistenFn | undefined;
    let onAbort: (() => void) | undefined;

    try {
      if (options.onProgress) {
        const onProgress = options.onProgress;
        unlistenProgress = await listen<number>(`conv-progress:${jobId}`, (event) => {
          onProgress(event.payload);
        });
      }
      if (options.signal) {
        onAbort = () => {
          // Fire-and-forget: cancellation is best-effort (see this file's header on the command
          // contract), and a rejected cancel request shouldn't surface as an unhandled rejection
          // on top of whatever error the conversion itself ends with.
          invoke('cancel_conversion', { jobId }).catch(() => undefined);
        };
        options.signal.addEventListener('abort', onAbort, { once: true });
      }

      const output = await invoke<Uint8Array | number[]>('convert', {
        jobId,
        input,
        from,
        to,
        maxInputBytes: options.maxInputBytes,
      });
      return output instanceof Uint8Array ? output : Uint8Array.from(output);
    } catch (error) {
      throw fromBackendError(error);
    } finally {
      unlistenProgress?.();
      if (onAbort) {
        options.signal?.removeEventListener('abort', onAbort);
      }
    }
  }

  async listFormats(): Promise<FormatInfo[]> {
    try {
      return await invoke<FormatInfo[]>('list_formats');
    } catch (error) {
      throw fromBackendError(error);
    }
  }

  getMemoryCeilingBytes(): Promise<number | null> {
    // The native path is an ordinary OS process, not a 32-bit WASM linear memory — there's no
    // fixed ceiling to report. Real limits are whatever the OS/available RAM allow, which isn't
    // a constant this interface can usefully expose. Not `async` (nothing to `await`) — a plain
    // resolved promise satisfies the `ConvEngine` interface just as well.
    return Promise.resolve(null);
  }
}
