# Building conv.cat

conv.cat is two build systems sharing one repository, bridged by a WebAssembly
artifact. This document covers what that means in practice: the prerequisites, the
one command that builds everything, how to build individual pieces while you work,
and the failures that actually happen.

If you only want to get a working checkout: install the prerequisites, then run
`./scripts/build-all.sh`.

## Prerequisites

| Tool | Version | How to install | Notes |
| --- | --- | --- | --- |
| Node | see [`.nvmrc`](../.nvmrc) | `nvm install` | `nvm use` picks up `.nvmrc` automatically |
| pnpm | see `packageManager` in [`package.json`](../package.json) | `corepack enable` | npm/yarn will not work — this is a pnpm workspace |
| Rust | see [`rust-toolchain.toml`](../rust-toolchain.toml) | [rustup.rs](https://rustup.rs) | rustup reads the pinned toolchain automatically |
| wasm-pack | see [`.wasm-pack-version`](../.wasm-pack-version) | `cargo install wasm-pack --locked --version "$(cat .wasm-pack-version)"` | needed for the browser engine |

Every toolchain is pinned in-repo, so `nvm use` and `rustup` give you the same
versions CI uses. The `wasm32-unknown-unknown` target is declared in
`rust-toolchain.toml`; `build-all.sh` adds it if it is somehow missing.

**wasm-pack is pinned by file, not by toolchain.** Unlike Node and Rust it has no
`.nvmrc`-style mechanism, so the version lives in `.wasm-pack-version` and
`build-all.sh` enforces it. This is not version pedantry: wasm-pack bundles a
`wasm-bindgen` CLI that must match the `wasm-bindgen` **crate** version in
`crates/conv-wasm/Cargo.toml`. When they diverge you get a confusing runtime error
in the browser rather than an honest build failure, so it is worth catching up
front.

A mismatch **warns locally and fails under CI** — the same split as
`--frozen-lockfile`. You are never blocked by a version bump while working, but the
artifact CI produces is always built with the pinned version. To bump it: update
`.wasm-pack-version`, run the install command above, and verify with a full
`./scripts/build-all.sh --clean --check` in the same commit.

You do not need to install these individually — `build-all.sh` checks all four up
front and tells you exactly what is missing and how to get it, rather than dying
partway through with a toolchain error.

## The one command

```bash
./scripts/build-all.sh
```

| Flag | Effect |
| --- | --- |
| *(none)* | Debug build of everything. Fastest; what you want while developing. |
| `--release` | Optimised build. Slower, and what actually gets shipped. |
| `--check` | Additionally run `cargo fmt --check`, clippy, Rust tests, `pnpm lint`, JS typecheck, and the licence-boundary check. With `--release`, also the WASM size budget. |
| `--clean` | Remove all build outputs first. |
| `--help` | Usage. |

Flags combine. `./scripts/build-all.sh --release --check` is the full pre-release
gate and reproduces what CI does.

**Run `--check` before pushing.** It is the same set of gates CI enforces, so
catching a clippy warning locally costs you thirty seconds instead of a round trip
through a red PR.

## Build order, and why it matters

```
crates/*  ──cargo──▶  conv-wasm  ──wasm-pack──▶  packages/*  ──▶  apps/*
```

The order is not arbitrary, which is the entire reason `build-all.sh` exists
instead of a README note telling you to run four commands:

1. **`crates/*` (cargo)** — `conv-core` is the conversion engine. Everything
   downstream is compiled from it.
2. **`conv-wasm` (wasm-pack)** — compiles `conv-core` to WebAssembly, emitting an
   ES module into `crates/conv-wasm/pkg/`. Must come after step 1.
3. **`packages/*` (tsc)** — `packages/engine` consumes the WASM artifact and
   decides at runtime whether to use it or the native path. Must come after step 2.
4. **`apps/*` (tsc)** — UI over `packages/engine`. Must come last.

Build these out of order and you either link against a stale `.wasm` or fail on a
missing import. `pnpm -r` handles steps 3 and 4 correctly on its own — it walks the
workspace in topological order — but nothing except this script sequences the Rust
and JS halves against each other.

## Building individual pieces

While working on one area, the full build is usually unnecessary.

```bash
# Rust engine only — the fast inner loop for converter work
cargo check                     # fastest: type-checks without producing binaries
cargo test --workspace          # run the conversion tests
cargo clippy --workspace --all-targets -- -D warnings

# WebAssembly artifact only (after changing conv-core or conv-wasm)
wasm-pack build crates/conv-wasm --target web --out-dir pkg --dev

# JS side only
pnpm install
pnpm build                      # all packages and apps, topological order
pnpm typecheck
pnpm --filter @conv.cat/engine build     # one workspace member
```

For converter work in `crates/conv-core`, `cargo check` and `cargo test` are the
whole loop — you do not need to rebuild the WASM artifact or the JS side until you
want to see it in the app.

## Build outputs

| Path | Produced by | Committed? |
| --- | --- | --- |
| `target/` | cargo | No — gitignored |
| `crates/conv-wasm/pkg/` | wasm-pack | No — gitignored |
| `packages/*/dist/`, `apps/*/dist/` | tsc | No — gitignored |
| `node_modules/` | pnpm | No — gitignored |

All build outputs are gitignored. If `git status` shows any of these, something
generated an artifact outside its expected location — worth investigating rather
than committing.

`build-all.sh` prints the size of `conv_wasm_bg.wasm` on every run. That payload
ships to every visitor, so the number is surfaced deliberately: a regression should
be visible while you are building, not discovered later. A CI budget that fails the
build on regression is [scoped in the roadmap](ROADMAP.md).

## Troubleshooting

**`pnpm install` fails with a lockfile error.** In CI the script uses
`--frozen-lockfile`, so a lockfile that drifted from `package.json` fails the build
rather than being silently rewritten. Locally the script allows the update — run
`pnpm install` and commit the resulting `pnpm-lock.yaml`.

**`wasm-pack` fails on a missing target.** `rustup target add wasm32-unknown-unknown`.
The script does this automatically when rustup is present; a non-rustup Rust
install needs it done by hand.

**`cargo fmt --check` fails in `--check` mode.** Run `cargo fmt --all` and commit.
The check is intentionally non-fixing so that formatting never gets rewritten
underneath you mid-build.

**Clippy fails on code you did not touch.** Clippy runs with `-D warnings`, so
anything it flags is a build failure. That is deliberate: this repo starts clean
and never accumulates a lint backlog. If a rule is genuinely wrong for this
codebase, change the rule in a separate commit with a reason — do not add
`#[allow]` in passing.

**The licence-boundary check fails.** Something in `crates/*` or `packages/*`
imported from `apps/*`, which would pull AGPL code into the MIT half of the repo.
See [`ARCHITECTURE.md`](ARCHITECTURE.md) and
[`.github/scripts/check-licence-boundary.sh`](../.github/scripts/check-licence-boundary.sh).
The fix is to move the shared code down into a package or crate, never to relax the
check.

**Stale build after switching branches.** `./scripts/build-all.sh --clean`.

## What this does not build yet

`apps/web` and `apps/desktop` are scaffolds — `tsc` compiles them, but there is no
dev server and no runnable application. `pnpm dev` does not exist yet. Until the web
MVP lands, `build-all.sh` is how you verify a checkout is healthy. See
[`ROADMAP.md`](ROADMAP.md) for what is actually shipped versus scoped.
