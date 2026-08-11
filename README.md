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

## License

Split license, matching the layout above: apps are **AGPL-3.0-only**, crates and packages are
**MIT**. This keeps the reusable engine and libraries permissively licensed while requiring
that any hosted fork of the apps shares its source.

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
