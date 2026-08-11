# conv.cat units demo

A manual QA page proving the units vertical slice end to end: a real category/unit conversion,
run through Rust (`conv-core`) → WASM (`conv-wasm`) → `@conv.cat/engine`'s `convertUnit()`
convenience wrapper, with unit metadata read from `@conv.cat/data`. **This is not the product
UI** — `apps/web/src` is still a bare scaffold (the real Next.js single-page converter shell is a
separate, not-yet-built backlog ticket). This exists so the units architecture this ticket shipped
has a runnable proof in a real browser, not just tests — mirroring
`packages/engine/demo`, which does the same thing for the base pipeline.

Deliberately crude (no design, no framework, inline styles) so nobody mistakes it for the real
thing.

## Running it

From the repo root:

```bash
./scripts/build-all.sh                                       # or, minimally, the two lines below
pnpm --filter @conv.cat/data --filter @conv.cat/engine build
pnpm --filter @conv.cat/web demo
```

Then open the printed URL (`http://localhost:8788/` by default). Pick a category (try
`temperature` — it's the one that proves the offset math, e.g. Celsius `0` → Fahrenheit), a
`From`/`To` unit, a value, and hit Convert.

## Why this server serves the whole repo, not just this directory

Unlike `packages/engine/demo` (which only ever needs its own package's `dist/`), this page's
import map points at two *different* packages' built output —
`/packages/engine/dist/index.js` and `/packages/data/dist/index.js` — because a units page
genuinely needs both the engine and the catalog. The browser's ES module loader only understands
http(s) and relative URLs (no Node-style `node_modules` resolution in a plain
`<script type="module">`), so `serve.mjs` serves the repo root over plain HTTP instead of just
`apps/web/`. See `serve.mjs`'s own comments for the worker-import-map rewrite it also carries over
from `packages/engine/demo/serve.mjs` (import maps don't apply inside the module Worker
`@conv.cat/engine`'s WASM backend spawns — verified there, still true here).

## What this deliberately doesn't cover

- The native/Tauri path — same gap `packages/engine/demo` documents; `TauriBackend` is real and
  typechecked, `apps/desktop` isn't bootstrapped yet.
- Every unit category — only the eight this ticket ported (`length`, `mass`, `volume`, `cooking`,
  `temperature`, `fuel_consumption`, `life_age`, `clothing_size`). The category dropdown is driven
  by `@conv.cat/data`'s `UNIT_CATEGORIES`, so it grows automatically as more categories land — no
  change needed here.
- Pretty number formatting — the result is the raw wire-protocol text `conv-core` returns (full
  `f64` precision), not a locale-formatted display string. See
  `crates/conv-core/src/formats/units/mod.rs`'s module docs for why that boundary is deliberate.
