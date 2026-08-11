import type { ConvertErrorKind } from './types.js';

const KNOWN_KINDS = new Set<ConvertErrorKind>([
  'unsupported_pair',
  'malformed_input',
  'unsupported_feature',
  'size_limit_exceeded',
  'cancelled',
  'internal',
  'unknown_format',
  'memory_limit_exceeded',
  'backend_unavailable',
  'unknown',
]);

function isKnownKind(value: unknown): value is ConvertErrorKind {
  return typeof value === 'string' && KNOWN_KINDS.has(value as ConvertErrorKind);
}

/**
 * A conversion failure. `kind` is what call sites should switch on to decide what happened and
 * localize a message; `message` is a developer-facing fallback (English, not localized, not for
 * end-user display — same rule `conv-core::ConvertError`'s `Display` impl documents) and
 * `details` carries whatever typed data came with the error (e.g. `from`/`to` format ids for
 * `unsupported_pair`, `limit`/`actual` for `size_limit_exceeded`).
 */
export class ConvertError extends Error {
  readonly kind: ConvertErrorKind;
  readonly details: Readonly<Record<string, unknown>>;

  constructor(kind: ConvertErrorKind, message: string, details: Record<string, unknown> = {}) {
    super(message);
    this.name = 'ConvertError';
    this.kind = kind;
    this.details = details;
  }
}

/**
 * Shape conv-wasm throws for every error `convert()`/`supportedFormats()` can produce — see
 * `crates/conv-wasm/src/error.rs`. Structurally checked (not `instanceof`, the value crossed the
 * WASM boundary as a plain object) rather than assumed, because anything that isn't this crate's
 * own typed error (a real bug throwing a `TypeError`, `WebAssembly.RuntimeError` from a genuine
 * trap) can also legitimately reach this catch block.
 */
interface TypedErrorShape {
  kind: unknown;
  message: unknown;
  [key: string]: unknown;
}

function isTypedErrorShape(value: unknown): value is TypedErrorShape {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    'message' in value &&
    // TS's `in` narrowing above already gives `value.message` type `unknown` here — no cast
    // needed to reach it, only to confirm its runtime type.
    typeof value.message === 'string'
  );
}

/**
 * Rebuilds a {@link ConvertError} from whatever the WASM backend's worker (or, in a future
 * version once the desktop app implements it, the Tauri `convert` command — see
 * `packages/engine/README.md` "The Tauri contract" — sharing the same `{ kind, message,
 * ...details }` shape is the whole point of documenting it) actually threw. Anything that isn't
 * that shape still becomes a `ConvertError`, tagged `internal`, so call sites only ever need to
 * catch one error type regardless of backend or failure mode.
 */
export function fromBackendError(value: unknown): ConvertError {
  if (value instanceof ConvertError) {
    return value;
  }
  if (isTypedErrorShape(value)) {
    const { kind, message, ...details } = value;
    // `isTypedErrorShape` already verified `message` is a `string` at runtime; the field stays
    // typed `unknown` on `TypedErrorShape` itself (matching an untrusted cross-boundary value),
    // so `String()` here is just satisfying the type checker with what's already guaranteed.
    return new ConvertError(isKnownKind(kind) ? kind : 'unknown', String(message), details);
  }
  const message = value instanceof Error ? value.message : String(value);
  return new ConvertError('internal', message, { cause: value });
}
