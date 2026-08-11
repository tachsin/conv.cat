#!/usr/bin/env node
// Generates `crates/conv-core/src/formats/units/generated.rs` from the six "generic model"
// unit categories in `packages/data/src/units/categories/` — the ones whose conversion math is
// pure per-unit factor/scale/offset/reciprocal arithmetic (`length`, `mass`, `volume`, `cooking`,
// `temperature`, `fuel_consumption`). `life_age` and `clothing_size` are hand-ported algorithms
// (piecewise curves, measurement charts), not table-driven, so they're deliberately not part of
// this codegen — see crates/conv-core/src/formats/units/{life_age,clothing_size}.rs.
//
// Run after editing any of those six category JSON files:
//   pnpm generate:units
// then review the diff in generated.rs before committing — see that file's own header.
//
// Why this script exists instead of hand-transcribing ~285 numbers into Rust: `packages/data`'s
// JSON is the single source of truth for the catalog, and hand-copying that many factors into Rust
// syntax is exactly the kind of transcription a legacy port has to get bit-for-bit right. The two
// non-linear categories here (`temperature`'s affine math, `fuel_consumption`'s reciprocal math)
// aren't expressible as a plain JSON number in the legacy source — only as a formula string like
// `"(x + 459.67) * 5/9"` — so rather than writing a general formula parser for a handful of cases,
// this script has a small explicit override table below, each entry commented with the exact
// legacy formula string it was derived from, so the derivation is checkable by inspection.

import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const repoRoot = fileURLToPath(new URL('..', import.meta.url));
const dataDir = join(repoRoot, 'packages/data/src/units/categories');
const outPath = join(
  repoRoot,
  'crates/conv-core/src/formats/units/generated.rs',
);

// The six categories this codegen covers. Order here is the order emitted in generated.rs.
const GENERIC_MODEL_CATEGORIES = [
  'length',
  'mass',
  'volume',
  'cooking',
  'temperature',
  'fuel_consumption',
];

/**
 * Affine overrides: `to_si(x) = scale * x + offset`, `from_si(si) = (si - offset) / scale`.
 * Every entry is temperature — the only in-scope category with non-linear-through-origin formula
 * units. `scale`/`offset` are Rust source expressions (not pre-computed decimals), so `rustc`
 * evaluates the exact same arithmetic the legacy formula string describes instead of trusting a
 * hand-rounded decimal transcription.
 *
 * Source: conv.cat_legacy/conv-shared/data/units/categories/temperature.json (`to_kelvin`).
 */
const AFFINE_OVERRIDES = {
  'temperature.celsius': {
    scale: '1.0',
    offset: '273.15',
    formula: 'x + 273.15',
  },
  'temperature.delisle': {
    scale: '-2.0 / 3.0',
    offset: '373.15',
    formula: '373.15 - x * 2/3',
  },
  'temperature.fahrenheit': {
    scale: '5.0 / 9.0',
    offset: '459.67 * 5.0 / 9.0',
    formula: '(x + 459.67) * 5/9',
  },
  'temperature.kelvin': {
    scale: '1.0',
    offset: '0.0',
    formula: 'x',
  },
  'temperature.newton_temp': {
    scale: '100.0 / 33.0',
    offset: '273.15',
    formula: 'x * 100/33 + 273.15',
  },
  'temperature.planck_temp': {
    scale: '1.417e32',
    offset: '0.0',
    formula: 'x * 1.417e32',
  },
  'temperature.rankine': {
    scale: '5.0 / 9.0',
    offset: '0.0',
    formula: 'x * 5/9',
  },
  'temperature.reaumur': {
    scale: '5.0 / 4.0',
    offset: '273.15',
    formula: 'x * 5/4 + 273.15',
  },
  'temperature.romer': {
    scale: '40.0 / 21.0',
    offset: '273.15 - 7.5 * 40.0 / 21.0',
    formula: '(x - 7.5) * 40/21 + 273.15',
  },
};

/**
 * Reciprocal overrides: `to_si(x) = k / x`, `from_si(si) = k / si` — a self-inverse (involution)
 * transform. The legacy JSON's `to_base`/`from_base` strings are identical for every one of these
 * (`"100/x"` etc.), which is exactly what makes them representable as a single constant `k` rather
 * than needing separate to/from formulas.
 *
 * Source: conv.cat_legacy/conv-shared/data/units/categories/fuel_consumption.json (`to_base`).
 */
const RECIPROCAL_OVERRIDES = {
  'fuel_consumption.km_per_liter': { k: '100.0', formula: '100/x' },
  'fuel_consumption.mile_per_liter': {
    k: '100.0 / 1.60934',
    formula: '100/1.60934/x',
  },
  'fuel_consumption.mpg_uk': { k: '282.481', formula: '282.481/x' },
  'fuel_consumption.mpg_us': { k: '235.215', formula: '235.215/x' },
};

function rustStringLit(s) {
  return JSON.stringify(s);
}

async function loadCategory(id) {
  const raw = await readFile(join(dataDir, `${id}.json`), 'utf8');
  return JSON.parse(raw);
}

function emitUnit(categoryId, unit) {
  const key = `${categoryId}.${unit.id}`;
  const idLit = rustStringLit(unit.id);

  if (typeof unit.factor_to_si === 'number') {
    return `        UnitEntry { id: ${idLit}, conversion: UnitConversion::Linear { factor_to_si: ${formatRustFloat(unit.factor_to_si)} } },`;
  }
  const affine = AFFINE_OVERRIDES[key];
  if (affine) {
    return (
      `        // ${unit.id}: to_kelvin = ${affine.formula}\n` +
      `        UnitEntry { id: ${idLit}, conversion: UnitConversion::Affine { scale: ${affine.scale}, offset: ${affine.offset} } },`
    );
  }
  const reciprocal = RECIPROCAL_OVERRIDES[key];
  if (reciprocal) {
    return (
      `        // ${unit.id}: to_base = ${reciprocal.formula}\n` +
      `        UnitEntry { id: ${idLit}, conversion: UnitConversion::Reciprocal { k: ${reciprocal.k} } },`
    );
  }
  return `        // ${unit.id}: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).\n        UnitEntry { id: ${idLit}, conversion: UnitConversion::Unconvertible },`;
}

function formatRustFloat(n) {
  if (!Number.isFinite(n)) {
    throw new Error(`non-finite factor_to_si: ${n}`);
  }
  // Rust needs a `.0` on whole-number float literals; JS's default stringification drops it.
  const s = String(n);
  return /[.eE]/.test(s) ? s : `${s}.0`;
}

async function main() {
  const categories = await Promise.all(
    GENERIC_MODEL_CATEGORIES.map(async (id) => [id, await loadCategory(id)]),
  );

  const tables = categories
    .map(([id, data]) => {
      const units = data.units.map((u) => emitUnit(id, u)).join('\n');
      return `    CategoryTable {\n        category_id: ${rustStringLit(id)},\n        units: &[\n${units}\n        ],\n    },`;
    })
    .join('\n');

  const generatedAt = new Date().toISOString();

  const out = `//! GENERATED — do not hand-edit.
//!
//! Source: \`packages/data/src/units/categories/{${GENERIC_MODEL_CATEGORIES.join(',')}}.json\`.
//! Regenerate with \`pnpm generate:units\` (runs \`scripts/generate-units-catalog.mjs\`) after
//! editing any of those files, then review the diff before committing — see
//! \`packages/data/src/units/README.md\` for the full regeneration rule and the honest list of
//! units with no conversion data (marked [\`UnitConversion::Unconvertible\`] below, each with a
//! comment explaining why).
//!
//! Generated ${generatedAt}.

use super::catalog::{CategoryTable, UnitConversion, UnitEntry};

/// Every "generic model" category's unit table — [\`super::converter::UnitsConverter\`] looks this
/// up by category id for every category except \`life_age\`/\`clothing_size\`, which are hand-ported
/// algorithms in their own modules instead of table data.
pub static CATEGORIES: &[CategoryTable] = &[
${tables}
];
`;

  await writeFile(outPath, out, 'utf8');
  console.log(`wrote ${outPath}`);
}

await main();
