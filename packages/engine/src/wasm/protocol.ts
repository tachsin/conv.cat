// The message shapes crossing the postMessage boundary between wasm/client.ts (main thread) and
// wasm/worker.ts (the Web Worker actually running conv-wasm). Internal to this package — not
// part of the public API in ../types.ts, and not meant to be stable across versions.

import type { FormatInfo } from '../types.js';

export interface ConvertRequest {
  type: 'convert';
  jobId: number;
  input: Uint8Array;
  from: string;
  to: string;
  maxInputBytes?: number;
}

export interface CancelRequest {
  type: 'cancel';
  jobId: number;
}

export interface ListFormatsRequest {
  type: 'list-formats';
  requestId: number;
}

export type WorkerRequest = ConvertRequest | CancelRequest | ListFormatsRequest;

/** The `{ kind, message, ...details }` shape conv-wasm throws — see `errors.ts#fromBackendError`. */
export interface WireError {
  kind: string;
  message: string;
  [key: string]: unknown;
}

export interface ProgressResponse {
  type: 'progress';
  jobId: number;
  fraction: number;
}

export interface ResultResponse {
  type: 'result';
  jobId: number;
  output: Uint8Array;
}

export interface ErrorResponse {
  type: 'error';
  jobId: number;
  error: WireError;
}

export interface ListFormatsResultResponse {
  type: 'list-formats-result';
  requestId: number;
  formats: FormatInfo[];
  /**
   * Piggybacked on this response rather than fetched separately: answering it needs the WASM
   * module loaded, `listFormats()` already forces that load, and a dedicated round trip would
   * either duplicate that load on the main thread (defeating the point of loading it only inside
   * the worker — see worker.ts) or add a message type purely to avoid piggybacking one number.
   */
  memoryCeilingBytes: number;
}

export interface ListFormatsErrorResponse {
  type: 'list-formats-error';
  requestId: number;
  error: WireError;
}

export type WorkerResponse =
  | ProgressResponse
  | ResultResponse
  | ErrorResponse
  | ListFormatsResultResponse
  | ListFormatsErrorResponse;

/**
 * Picks the zero-copy transfer list for `postMessage`-ing a `Uint8Array`: transferable only when
 * it's backed by a plain (non-shared) `ArrayBuffer` that it covers exactly — a view offset into a
 * larger buffer, or a `SharedArrayBuffer`-backed array, falls back to structured-clone's normal
 * copy instead of risking transferring more (or less, or something already-shared) than intended.
 * The common case — bytes freshly read from a `File`/`Blob` via `new
 * Uint8Array(await blob.arrayBuffer())` — always qualifies, which is the case multi-hundred-MB
 * inputs actually hit.
 */
export function transferListFor(bytes: Uint8Array): Transferable[] {
  const buffer = bytes.buffer;
  const isPlainArrayBuffer = typeof ArrayBuffer !== 'undefined' && buffer instanceof ArrayBuffer;
  const coversWholeBuffer = bytes.byteOffset === 0 && bytes.byteLength === buffer.byteLength;
  return isPlainArrayBuffer && coversWholeBuffer ? [buffer] : [];
}
