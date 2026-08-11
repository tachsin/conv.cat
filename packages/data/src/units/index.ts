// Unit catalog: metadata only (id, display name, symbol, and — where the legacy source defined
// one — a linear factor or formula string). No conversion math here — that lives entirely in
// `crates/conv-core/src/formats/units/` (see this directory's README.md for the full rule and for
// which categories/units are actually convertible vs. catalog-only in this build).

import indexData from './index.json' with { type: 'json' };
import lengthData from './categories/length.json' with { type: 'json' };
import massData from './categories/mass.json' with { type: 'json' };
import volumeData from './categories/volume.json' with { type: 'json' };
import cookingData from './categories/cooking.json' with { type: 'json' };
import temperatureData from './categories/temperature.json' with { type: 'json' };
import fuelConsumptionData from './categories/fuel_consumption.json' with { type: 'json' };
import lifeAgeData from './categories/life_age.json' with { type: 'json' };
import clothingSizeData from './categories/clothing_size.json' with { type: 'json' };

/** One unit's catalog entry. `factor_to_si`/`to_kelvin`/`to_base` etc. are display/documentation
 * metadata carried over from the source catalog — `crates/conv-core` does not read this file at
 * runtime, so a unit having (or lacking) one of these fields here does not by itself determine
 * whether it's actually convertible; see this directory's README.md for the authoritative list of
 * gaps. */
export interface UnitDef {
  id: string;
  name: string;
  symbol?: string;
  factor_to_si?: number;
  to_kelvin?: string;
  from_kelvin?: string;
  to_base?: string;
  from_base?: string;
  note?: string;
  range?: string;
}

/** One unit category's full catalog. */
export interface UnitCategory {
  id: string;
  name: string;
  si_base?: string;
  conversion_type?: string;
  note?: string;
  categories?: string[];
  regions?: string[];
  units: UnitDef[];
}

const CATEGORY_DATA: Readonly<Record<string, UnitCategory>> = {
  length: lengthData,
  mass: massData,
  volume: volumeData,
  cooking: cookingData,
  temperature: temperatureData,
  fuel_consumption: fuelConsumptionData,
  life_age: lifeAgeData,
  clothing_size: clothingSizeData,
};

/** Every unit-category id this build's catalog ships, in catalog order — see README.md for scope
 * (a representative 8-category subset of the legacy catalog's 49). */
export const UNIT_CATEGORIES: readonly string[] = indexData.categories;

/** The `conv-core`/`conv-wasm` `Format` id for a unit category, e.g. `"length"` -> `"units_length"`.
 * Mirrors the `units_<category_id>` convention `Format::id()` uses on the Rust side
 * (`crates/conv-core/src/registry.rs`) — keep in sync if that convention ever changes; nothing
 * currently enforces the two stay aligned automatically. */
export function unitsFormatId(categoryId: string): string {
  return `units_${categoryId}`;
}

/** URL-friendly slug for a category id, matching the legacy app's `id.replace(/_/g, "-")`
 * convention (e.g. `"fuel_consumption"` -> `"fuel-consumption"`). */
export function urlSegmentForUnitCategory(categoryId: string): string {
  return categoryId.replace(/_/g, '-');
}

/** Looks up one category's full catalog (metadata only), or `undefined` if `categoryId` isn't one
 * of `UNIT_CATEGORIES`. */
export function getUnitCategory(categoryId: string): UnitCategory | undefined {
  return CATEGORY_DATA[categoryId];
}

/** Every in-scope category's full catalog, in `UNIT_CATEGORIES` order. */
export function listUnitCategories(): UnitCategory[] {
  const categories: UnitCategory[] = [];
  for (const id of UNIT_CATEGORIES) {
    const category = CATEGORY_DATA[id];
    if (category) categories.push(category);
  }
  return categories;
}
