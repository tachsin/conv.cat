// The public shape @conv.cat/engine exposes. Both backends (wasm/client.ts, tauri/client.ts)
// implement ConvEngine identically — this file, not either backend, is the contract the web app
// and the desktop app are allowed to depend on. See docs/ARCHITECTURE.md "One engine, two
// runtimes".

/**
 * Stable, machine-readable error identifiers — mirrors `conv_core::ConvertError`'s variants
 * (`unsupported_pair` through `internal`) plus a few this package's boundary can produce on its
 * own before ever reaching `conv-core`: `unknown_format` (an id neither backend recognizes),
 * `memory_limit_exceeded` (conv-wasm's 32-bit ceiling — see `WasmBackend`'s
 * `getMemoryCeilingBytes`), `backend_unavailable` (the detected backend's runtime isn't actually
 * usable, e.g. Workers disabled), and `unknown` (a shape this version of the package doesn't
 * recognize — `conv_core::ConvertError` is `#[non_exhaustive]`, so a newer conv-wasm build can
 * report a kind an older `@conv.cat/engine` hasn't been taught yet).
 *
 * These are identifiers, not user-facing text — same rule as the Rust side. Localize by
 * switching on `kind`, never by displaying `ConvertError.message` to an end user.
 */
export type ConvertErrorKind =
  | 'unsupported_pair'
  | 'malformed_input'
  | 'unsupported_feature'
  | 'size_limit_exceeded'
  | 'cancelled'
  | 'internal'
  | 'unknown_format'
  | 'memory_limit_exceeded'
  | 'backend_unavailable'
  | 'unknown';

/** Per-call knobs for {@link ConvEngine.convert}. Every field is optional. */
export interface ConvertOptions {
  /**
   * Called with a fraction in `0..=1` as the conversion makes headway. Not every converter can
   * report this evenly — a caller must not assume even spacing, or more than one call — matching
   * `conv_core::ProgressSink::on_progress`'s own contract.
   */
  onProgress?: (fraction: number) => void;

  /**
   * Requests cancellation when aborted. Honored at the underlying converter's next checkpoint,
   * not necessarily instantly — a converter with no internal checkpoints (like today's only
   * registered converter, the plain-text passthrough) can only be cancelled before it starts.
   * See `crates/conv-wasm/src/progress.rs` for the full same-thread-cooperative explanation on
   * the WASM backend.
   */
  signal?: AbortSignal;

  /**
   * Caps input size, in bytes, for this call. Both backends additionally enforce their own
   * ceiling regardless of this value — see {@link ConvEngine.getMemoryCeilingBytes} — this can
   * only make the effective limit *tighter*, never looser.
   */
  maxInputBytes?: number;
}

/** Metadata for one format this engine instance can read and/or write. */
export interface FormatInfo {
  /** Stable lowercase id, e.g. `"plain_text"` — pass this as `convert()`'s `from`/`to`. */
  id: string;
  category: string;
  mime: string;
  extensions: string[];
  canRead: boolean;
  canWrite: boolean;
}

export type EngineBackendKind = 'wasm' | 'tauri';

/**
 * The one interface the web app and the desktop app are allowed to depend on. Call
 * {@link getEngine} to get an instance — never construct `WasmBackend`/`TauriBackend` directly
 * from app code, that defeats the entire point of this package (see docs/ARCHITECTURE.md "Why
 * one engine").
 */
export interface ConvEngine {
  /** Which backend this instance is actually running — for diagnostics/telemetry only. Neither
   * app should branch product behavior on this; if you find yourself wanting to, that's a sign
   * the abstraction is leaking and the fix belongs in this package, not the call site. */
  readonly backend: EngineBackendKind;

  /**
   * Converts `input` from `from` to `to` (format ids — see {@link FormatInfo.id} /
   * {@link ConvEngine.listFormats}), resolving with the converted bytes or rejecting with a
   * {@link ConvertError}.
   *
   * **`input` is consumed.** On the WASM backend its underlying buffer is transferred
   * (zero-copy) to the Web Worker doing the conversion, not cloned — required to keep
   * multi-hundred-MB files off the UI thread without doubling memory use just to hand them over.
   * Both backends honor this as part of the contract (not just as an implementation detail of
   * one of them) so call sites stay portable: **do not read `input` again after calling this.**
   */
  convert(
    input: Uint8Array,
    from: string,
    to: string,
    options?: ConvertOptions,
  ): Promise<Uint8Array>;

  /** Every format this engine instance can convert. */
  listFormats(): Promise<FormatInfo[]>;

  /**
   * The hard input-size ceiling this backend enforces, in bytes, or `null` if it doesn't have a
   * fixed one. The WASM backend has one (32-bit linear memory — see `crates/conv-wasm`'s crate
   * docs for the exact number and why); the native Tauri path runs as an ordinary OS process and
   * isn't bound by it, so it reports `null`. `async` because the WASM backend only knows the
   * real number once its module has loaded — this package's laziness contract (see
   * `packages/engine/README.md` "Bundle size and lazy loading") means that shouldn't be assumed
   * to have happened yet.
   */
  getMemoryCeilingBytes(): Promise<number | null>;
}
