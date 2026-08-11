// Small, pure helpers over `FormatInfo` (from `@conv.cat/engine`, ultimately sourced from
// `crates/conv-wasm/src/format.rs::supported_formats()`). No conversion logic lives here — this
// only reshapes the catalog the engine already reports for the UI's own bookkeeping (grouping,
// guessing, labeling, output filenames). See docs/ARCHITECTURE.md "Typed errors and identifiers,
// not human strings" for why the engine hands back ids, not display text, and why turning those
// ids into UI copy is this app's job.

import type { FormatInfo } from '@conv.cat/engine';

/**
 * Formats this shell treats as file-shaped — the ones a drag-and-drop / file-picker flow makes
 * sense for. Filters on `extensions.length > 0` rather than a hardcoded id list, so it stays
 * correct with zero changes here as new file formats land in `conv-core`.
 *
 * Today's only non-file-shaped formats are the 8 `units_*` ids (`extensions: []`, see
 * `crates/conv-core/src/registry.rs`) — a unit conversion is `(value, from, to) -> value`, not a
 * file, and has its own UI paradigm (see `apps/web/units-demo`, not this shell's scope per the
 * "apps/web: the single-page converter shell" ticket).
 */
export function fileFormats(formats: FormatInfo[]): FormatInfo[] {
  return formats.filter((format) => format.extensions.length > 0);
}

/** File formats this engine build can decode. */
export function readableFormats(formats: FormatInfo[]): FormatInfo[] {
  return fileFormats(formats).filter((format) => format.canRead);
}

/** File formats this engine build can encode, restricted to one category (a conversion never
 * crosses categories — see `conv_core::registry::default_registry` pairing). */
export function writableFormatsInCategory(formats: FormatInfo[], category: string): FormatInfo[] {
  return fileFormats(formats).filter((format) => format.canWrite && format.category === category);
}

/**
 * Best-effort guess at a dropped file's format: by extension first (what a user actually sees in
 * a filename), falling back to the browser-reported MIME type. Returns `undefined` when neither
 * matches a known readable format — the caller lets the person pick manually rather than
 * guessing wrong, since a silently-wrong "detected" format would convert the wrong bytes.
 */
export function guessFormat(file: File, formats: FormatInfo[]): FormatInfo | undefined {
  const candidates = readableFormats(formats);
  const ext = extensionOf(file.name);
  if (ext) {
    const byExtension = candidates.find((format) => format.extensions.includes(ext));
    if (byExtension) return byExtension;
  }
  if (file.type) {
    const byMime = candidates.find((format) => format.mime === file.type);
    if (byMime) return byMime;
  }
  return undefined;
}

function extensionOf(filename: string): string | undefined {
  const dot = filename.lastIndexOf('.');
  if (dot <= 0 || dot === filename.length - 1) return undefined;
  return filename.slice(dot + 1).toLowerCase();
}

/** Turns a format id (`"plain_text"`) into display copy (`"Plain Text"`). The engine never
 * returns human strings itself (by design — see this file's header comment); this is that
 * boundary, kept in exactly one place. */
export function formatLabel(format: FormatInfo): string {
  return format.id
    .split('_')
    .map((word) => (word.length > 0 ? word[0].toUpperCase() + word.slice(1) : word))
    .join(' ');
}

/** The filename a converted download should carry: original basename, target format's primary
 * extension. */
export function outputFileName(originalName: string, target: FormatInfo): string {
  const dot = originalName.lastIndexOf('.');
  const base = dot > 0 ? originalName.slice(0, dot) : originalName;
  const ext = target.extensions[0];
  return ext ? `${base}.${ext}` : base;
}

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Human-readable byte size — display concern, not engine concern, same rule as unit-conversion
 * result formatting (see `@conv.cat/engine`'s units README section on that boundary). */
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), BYTE_UNITS.length - 1);
  const value = bytes / 1024 ** exponent;
  const rounded = exponent === 0 ? String(value) : value.toFixed(value < 10 ? 2 : 1);
  return `${rounded} ${BYTE_UNITS[exponent]}`;
}
