# conv.cat

A local-first file converter — units, images, video/audio, text/data, and CAD — built as a
pnpm + Cargo monorepo. The conversion engine is a framework-free Rust crate, compiled to WASM
for the web app and used natively in the desktop app, so both apps run the exact same
conversion code.

## Layout

```
conv.cat/
├─ apps/
│  ├─ web/          Next.js app            (AGPL-3.0-only)
│  └─ desktop/      Tauri app              (AGPL-3.0-only)
├─ crates/
│  ├─ conv-core/    Rust engine            (MIT)
│  └─ conv-wasm/    wasm-bindgen bindings  (MIT)
├─ packages/
│  ├─ engine/       TS wrapper: WASM in browser, native in Tauri (MIT)
│  ├─ media/        ffmpeg-wasm, stays TS  (MIT)
│  └─ data/         catalogs, units, timezones, i18n (MIT)
├─ docs/            community & architecture docs
└─ .github/         CI workflows, issue/PR templates
```

Every member has its own `README.md` — read it before touching that code; each one states
what the package is and what it must never contain (the ground rule is: **conversion logic
lives in `crates/conv-core`, nowhere else** — not in the apps, not in `packages/data`).

## Licence

conv.cat uses a split licence, matching the layout above:

- **MIT** — `crates/conv-core`, `crates/conv-wasm`, `packages/engine`, `packages/media`,
  `packages/data`. Reuse the conversion engine and libraries in your own software, including
  closed-source and commercial software.
- **AGPL-3.0-only** — `apps/web`, `apps/desktop`. Run a modified version of the apps as a
  network service and you must publish the corresponding source of your modifications.

Each of those directories has its own `LICENSE` file with the authoritative terms; the root
[`LICENSE`](LICENSE) maps which licence applies where.

Dependency direction matters here: `apps/*` may import the MIT packages and crates, but no
crate or package may ever depend on `apps/*` — that would pull AGPL code into the MIT half.
See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Trademark

The licences cover the code, not the brand. The name **"conv.cat"**, the **conv.cat domain**,
and the **conv.cat logo and mascot** are not granted by either licence and remain the property
of the project owner.

You are free to fork, modify and redistribute the code under the terms above, but a fork must
not present itself as conv.cat: pick your own name, your own domain and your own logo, and do
not imply that your build is the official one or endorsed by it. Referring to conv.cat
factually — "based on conv.cat", "a fork of conv.cat" — is fine.

## Getting started

Toolchains are pinned via `.nvmrc` and `rust-toolchain.toml` — use `nvm use` (or equivalent)
and let `rustup` pick up the pinned Rust toolchain automatically.

```bash
# JS side — one install bootstraps every package in apps/* and packages/*
pnpm install
pnpm build

# Rust side — one build covers every crate in crates/*
cargo build
cargo test
```

This repo is a fresh scaffold: the structure above exists and builds, but it is empty on
purpose. Conversion logic, the real app shells, and CI land in follow-up tickets tracked in
the project backlog.
