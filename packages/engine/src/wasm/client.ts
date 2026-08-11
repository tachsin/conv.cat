// Main-thread facade over the Web Worker in worker.ts. This is the piece `getEngine()` hands
// back when `detectBackend()` says `"wasm"` — the web app talks to this, never to worker.ts or
// `conv-wasm` directly.

import { fromBackendError } from '../errors.js';
import type { ConvEngine, ConvertOptions, FormatInfo } from '../types.js';
import type { WorkerRequest, WorkerResponse } from './protocol.js';
import { transferListFor } from './protocol.js';

/**
 * One Worker per page, created lazily on the first call that needs it (not at module import
 * time) and reused for every conversion after that — spinning up a fresh Worker (and re-fetching
 * the WASM module inside it) per call would throw away the whole point of lazy-loading it once.
 */
let sharedWorker: Worker | null = null;
let nextJobId = 1;
let nextRequestId = 1;

function getWorker(): Worker {
  sharedWorker ??= new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
  return sharedWorker;
}

export class WasmBackend implements ConvEngine {
  readonly backend = 'wasm' as const;

  private cachedMemoryCeilingBytes: number | null = null;

  async convert(
    input: Uint8Array,
    from: string,
    to: string,
    options: ConvertOptions = {},
  ): Promise<Uint8Array> {
    if (options.signal?.aborted) {
      throw fromBackendError({ kind: 'cancelled', message: 'cancelled before it started' });
    }

    const worker = getWorker();
    const jobId = nextJobId++;
    const transfer = transferListFor(input);

    return new Promise<Uint8Array>((resolve, reject) => {
      const onAbort = () => {
        const cancel: WorkerRequest = { type: 'cancel', jobId };
        worker.postMessage(cancel);
      };
      options.signal?.addEventListener('abort', onAbort, { once: true });

      const cleanup = () => {
        worker.removeEventListener('message', onMessage);
        options.signal?.removeEventListener('abort', onAbort);
      };

      const onMessage = (event: MessageEvent<WorkerResponse>) => {
        const response = event.data;
        if (!('jobId' in response) || response.jobId !== jobId) {
          return;
        }
        if (response.type === 'progress') {
          options.onProgress?.(response.fraction);
          return;
        }
        cleanup();
        if (response.type === 'result') {
          resolve(response.output);
        } else if (response.type === 'error') {
          reject(fromBackendError(response.error));
        }
      };
      worker.addEventListener('message', onMessage);

      const request: WorkerRequest = {
        type: 'convert',
        jobId,
        input,
        from,
        to,
        maxInputBytes: options.maxInputBytes,
      };
      worker.postMessage(request, transfer);
    });
  }

  async listFormats(): Promise<FormatInfo[]> {
    const response = await this.requestListFormats();
    return response.formats;
  }

  async getMemoryCeilingBytes(): Promise<number | null> {
    // Answering this needs the WASM module loaded, which only ever happens inside the worker
    // (see worker.ts) — importing conv-wasm here on the main thread just to read one constant
    // would load a second, separate copy of the module and defeat that entirely. list-formats
    // already forces the load and now carries the ceiling piggybacked on its response (see
    // protocol.ts), so this reuses that instead of adding a redundant round trip.
    if (this.cachedMemoryCeilingBytes === null) {
      const response = await this.requestListFormats();
      this.cachedMemoryCeilingBytes = response.memoryCeilingBytes;
    }
    return this.cachedMemoryCeilingBytes;
  }

  private async requestListFormats(): Promise<{ formats: FormatInfo[]; memoryCeilingBytes: number }> {
    const worker = getWorker();
    const requestId = nextRequestId++;
    return new Promise((resolve, reject) => {
      const onMessage = (event: MessageEvent<WorkerResponse>) => {
        const response = event.data;
        if (
          (response.type !== 'list-formats-result' && response.type !== 'list-formats-error') ||
          response.requestId !== requestId
        ) {
          return;
        }
        worker.removeEventListener('message', onMessage);
        if (response.type === 'list-formats-result') {
          resolve({ formats: response.formats, memoryCeilingBytes: response.memoryCeilingBytes });
        } else {
          reject(fromBackendError(response.error));
        }
      };
      worker.addEventListener('message', onMessage);
      const request: WorkerRequest = { type: 'list-formats', requestId };
      worker.postMessage(request);
    });
  }
}
