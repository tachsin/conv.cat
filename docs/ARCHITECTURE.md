# Architecture

> **Stub.** The full architecture document (conversion pipeline, WASM boundary, app shells) is
> written in the "Write the community docs set" ticket. This file currently carries only the
> dependency rules, which are load-bearing for the licence split and must not be lost.

## Dependency direction

The repository is licensed in two halves (see [`../LICENSE`](../LICENSE)):

| Half | Directories | Licence |
| --- | --- | --- |
| Libraries | `crates/conv-core`, `crates/conv-wasm`, `packages/engine`, `packages/media`, `packages/data` | MIT |
| Applications | `apps/web`, `apps/desktop` | AGPL-3.0-only |

**Dependencies flow one way only: apps depend on libraries, never the reverse.**

- ✅ `apps/web` importing `@conv.cat/engine`, `@conv.cat/media`, `@conv.cat/data`.
- ✅ `apps/desktop` depending on `conv-core` / `conv-wasm`.
- ✅ `crates/conv-wasm` depending on `crates/conv-core`; `packages/engine` depending on the
  crates' WASM output.
- ❌ **Any** crate or package depending on `apps/*` — a path dependency, a workspace import, a
  relative `../../apps/...` import, a type imported from an app, or a test fixture reached into
  from `apps/`.

### Why this is not just tidiness

The MIT half exists so third parties can embed the conversion engine in their own software,
including closed-source software. The moment a library imports anything from `apps/*`, it is
a derivative work of AGPL code, and the MIT grant on that library becomes a promise the project
cannot keep. A contributor who adds one convenience import breaks the licence guarantee for
every downstream user — silently, because the code still compiles.

### If you need something that lives in an app

Move it down, do not reach up. Shared logic belongs in the layer that both sides can depend on:

- conversion logic → `crates/conv-core` (framework-free, browser-free — see its README);
- catalogs, units, static data → `packages/data`;
- browser/native engine glue → `packages/engine`.

If it genuinely cannot move down, it is app-specific and should be duplicated per app rather
than shared upward.

### Enforcement

This rule is checked in CI by [`.github/workflows/licence-boundary.yml`](../.github/workflows/licence-boundary.yml),
which runs [`.github/scripts/check-licence-boundary.sh`](../.github/scripts/check-licence-boundary.sh)
on every push to `main` and every pull request. It catches Rust path dependencies, app packages
listed in a library's `package.json`, imports crossing the boundary by package name or relative
path, and files under `crates/`/`packages/` pointing at an app directory. Markdown is exempt:
a README saying "consumed by `apps/web`" describes the correct direction and is not a dependency.

Run it locally before pushing — it needs no toolchain and takes under a second:

```bash
./.github/scripts/check-licence-boundary.sh
```
