// Units conversion — a thin convenience layer over `ConvEngine.convert()` for the pipe-delimited
// text protocol `conv-core`'s units converter speaks (see
// crates/conv-core/src/formats/units/mod.rs). Deliberately NOT part of the `ConvEngine` interface
// itself (types.ts) — this format needs nothing from that contract that isn't already there (no
// new `ConvertOptions` field), so this stays additive rather than a contract change; both backends
// already work with it unmodified.

import { unitsFormatId } from '@conv.cat/data';

import type { ConvEngine, ConvertOptions } from './types.js';

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: false });

/**
 * Converts `value` from `fromUnitId` to `toUnitId` within `categoryId`, via `engine`.
 *
 * `value` is a `number` for every in-scope category except `clothing_size`, which also accepts a
 * size-label string (`"M"`, `"XL"`) — see `crates/conv-core/src/formats/units/clothing_size.rs`.
 * The result is likewise a `number` when the wire response parses as one, or the raw label string
 * otherwise (also only possible for `clothing_size`).
 *
 * Rejects with a {@link import('./errors.js').ConvertError} — same as
 * {@link ConvEngine.convert} itself; both backends already normalize errors into that shape, so
 * this function doesn't need its own error handling.
 */
export async function convertUnit(
  engine: ConvEngine,
  categoryId: string,
  fromUnitId: string,
  toUnitId: string,
  value: number | string,
  options?: ConvertOptions,
): Promise<number | string> {
  const format = unitsFormatId(categoryId);
  const payload = `${fromUnitId}|${toUnitId}|${value}`;
  const input = encoder.encode(payload);

  const output = await engine.convert(input, format, format, options);
  const text = decoder.decode(output);

  const asNumber = Number(text);
  return text !== '' && Number.isFinite(asNumber) ? asNumber : text;
}
