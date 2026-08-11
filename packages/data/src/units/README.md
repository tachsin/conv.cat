# Units catalog

Static unit-of-measurement metadata (id, display name, symbol, and — where the legacy source
defined one — a conversion factor or formula string) consumed by `crates/conv-core`'s codegen
script and by app UI code. **This is data, not logic** — the actual conversion math lives in
`crates/conv-core/src/formats/units/`, per this package's own rule (see the root `README.md`).

## What's in scope today

Eight categories, ported from `conv.cat`'s pre-OSS legacy codebase
(`conv-shared/data/units/categories/`) as a representative subset — see the backlog ticket "First
vertical slice: port units to conv-core" for why these eight and not the full legacy catalog (49
categories, ~977 units): `length`, `mass`, `volume`, `cooking`, `temperature`, `fuel_consumption`,
`life_age`, `clothing_size`.

The remaining ~41 legacy categories (`area`, `energy`, `pressure`, `currency`, `hardness`,
`shoe_size`, and so on) are not ported yet — most are pure linear-factor math like `length`/`mass`
and are expected to be a fast, mechanical follow-up once this pattern has shipped and been
reviewed.

## Honest gaps within the in-scope categories

Porting this data surfaced units that were **never actually convertible in the legacy app either**
— the legacy catalog JSON defines a display symbol and sometimes a formula string for them, but
neither the legacy Rust (`conv-rust/src-tauri/src/units/convert.rs`) nor the legacy TypeScript
(`conv-next/lib/unit-convert.ts`) ever parsed those formula strings; both only ever read
`factor_to_si`. `crates/conv-core`'s converter preserves this honestly: these units are catalogued
here (so they still show up for browsing/display) but a conversion request naming one returns a
typed `ConvertError::UnsupportedFeature`, never a fabricated result.

- **`temperature`**: `gas_mark` (oven gas mark), `triple_point_water` — no `to_kelvin`/`from_kelvin`
  formula in the source data.
- **`fuel_consumption`**: `gallon_per_100mile_us`, `gallon_per_100mile_uk`, `liter_per_km`,
  `liter_per_mile` — no `factor_to_si` and no `to_base`/`from_base` formula.
- **`clothing_size`**: `us_kids`, `us_toddler`, `us_infant`, `uk_kids`, `eu_kids`, `bra_us`,
  `bra_uk`, `bra_eu`, `bra_fr`, `bra_jp` — the legacy chart-based conversion logic
  (`conv-next/lib/clothing-size.ts`) has no chart data for kids or bra sizing; every other unit in
  this category (women's/men's tops via the bust/chest chart, hats, gloves, and the linear
  cm↔inch measurement pairs) is fully convertible.

Every other unit in these eight categories — including all 47 `life_age` species (cat/dog years
included) and every `temperature`/`fuel_consumption` unit not listed above — converts for real.
`temperature`'s Celsius/Fahrenheit/Rankine/etc. math and `fuel_consumption`'s reciprocal
(km-per-liter-style) math are implemented properly here for the first time; the legacy app never
actually computed either despite shipping the catalog data for them.

## Regenerating the Rust catalog

`crates/conv-core/src/formats/units/generated.rs` is generated from the six "generic model"
category files here (`length`, `mass`, `volume`, `cooking`, `temperature`, `fuel_consumption` —
`life_age` and `clothing_size` are hand-ported algorithms, not table-driven, so they're not part of
this codegen). After editing a category JSON file in this directory, regenerate with:

```bash
pnpm generate:units
```

then review the diff in `generated.rs` before committing — see that file's own header comment.
