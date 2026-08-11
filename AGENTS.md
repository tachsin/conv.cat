# AGENTS.md

Instructions for AI coding agents working in this repository.

This file is for agents *operating on* the codebase. It is not the policy for humans submitting
AI-assisted PRs — that is [`docs/ai-contributions.md`](docs/ai-contributions.md), and it still
applies to whoever opens the PR.

Read this file first. It is deliberately short and covers the things that are non-obvious,
invariant, or have already gone wrong once. Everything else lives in the docs linked at the
bottom.

## What this repo is

conv.cat converts files entirely on the user's device — no upload, no server. One Rust engine
(`crates/conv-core`) compiled two ways: to WebAssembly for the browser, and linked natively into
the desktop app. The apps are UI only.

This is a from-scratch rebuild in progress. Most of the format catalog does **not** work yet.
Check [`README.md` § Status](README.md#status-of-this-repository) before claiming anything is
shipped — that section is kept honest on purpose, and it is the one place stating what actually
exists today.

## The one command

```bash
./scripts/build-all.sh --check            # build everything + fmt, clippy, tests, lint, typecheck, licence
./scripts/build-all.sh --release --check  # ...and the WASM size budget. The full pre-push gate.
```

Run it before you claim work is done. It reproduces what CI enforces, so a green run locally is
the difference between finishing and a red PR. Individual commands:

| Task | Command |
| --- | --- |
| Rust tests (incl. the conformance suite) | `cargo test --workspace` |
| Rust lint — must be clean | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust formatting | `cargo fmt --all` (`--check` to verify) |
| JS/TS lint | `pnpm lint` (`pnpm lint:fix` to autofix) |
| JS/TS typecheck | `pnpm typecheck` |
| Licence boundary | `./.github/scripts/check-licence-boundary.sh` |
| WASM size budget | `./.github/scripts/check-wasm-size.sh` (needs a `--release` wasm build first) |

There is **no `pnpm dev`** — the apps have no UI yet. To see conversion actually working in a
browser, use the manual QA harnesses: `packages/engine/demo/` and `apps/web/units-demo/`. Neither
is product UI; do not build features on top of them.

## Hard rules

Violating any of these breaks something that will not be caught by reading the diff alone.

1. **Conversion logic lives in `crates/conv-core`. Nowhere else.** Not in the apps, not in
   `packages/data` (catalogs only), not in `packages/engine` (it is a seam, not an engine). If you
   are writing a parser, encoder, or unit table anywhere else, it is in the wrong place. The one
   deliberate exception is video/audio, which stays on ffmpeg-wasm in `packages/media` — see
   [ARCHITECTURE.md § the media boundary](docs/ARCHITECTURE.md#the-media-boundary-video-and-audio-stay-on-ffmpeg-wasm).

2. **Dependencies flow one way: `apps/*` → `packages/*`/`crates/*`, never the reverse.** This is
   what makes the split licence (MIT libraries, AGPL-3.0-only apps) truthful rather than a promise
   the project cannot keep. Enforced by `check-licence-boundary.sh` in CI.

3. **Errors are typed data, never English prose.** `ConvertError` carries variants and
   `&'static str` identifiers. UI text is the app's job via i18n. Do not add a `String` message
   field as the primary representation of a failure.

4. **`Format`, `Category` and `ConvertError` are `#[non_exhaustive]`.** Match with a wildcard arm.
   Adding a variant must stay a non-breaking change for downstream crates.

5. **Build order matters and is not negotiable:** `crates/*` → `conv-wasm` (wasm-pack) →
   `packages/*` → `apps/*`. `packages/engine` has a real `file:` dependency on the wasm-pack
   output, so `pnpm install` has nothing to link against until the WASM artifact exists. Use
   `build-all.sh` rather than running the four toolchains by hand.

6. **Never commit secrets, and never commit fixture files you did not generate.** Golden fixtures
   must be self-generated or CC0 — this is a public repo and random files off the web are a
   licensing problem.

## Traps that have already caught someone

Each of these looks like a bug and is not. Do not "fix" them.

- **The root `package.json` pins `typescript@6` while every package pins `typescript@7`. This is
  intentional.** typescript-eslint refuses to load under TS 7 — a hard runtime error, not a peer
  warning, so `pnpm lint` dies outright. The root TS 6 is the linter's parser only; each package
  builds with its own TS 7, and pnpm's isolated `node_modules` keeps them apart. Aligning the
  versions breaks lint. Full reasoning is at the top of [`eslint.config.js`](eslint.config.js).

- **A Rust doc comment containing the literal string `apps/web` or `apps/desktop` fails the
  licence-boundary check**, even in prose describing the correct dependency direction. Write "the
  desktop app" instead of the literal path token in `.rs` comments.

- **`console.log` lints clean in `apps/*` but fails in `packages/*`.** The apps include the `DOM`
  lib; the packages are environment-agnostic libraries and deliberately do not. A shared library
  reaching for a host global *should* be flagged. Do not add `DOM` to a package's tsconfig to
  silence it.

- **Regenerating a golden file is not a routine fix.** If output legitimately changed, regenerate
  with `UPDATE_GOLDENS=1 cargo test -p conv-core --test golden`, then `git diff` it and justify
  the change in the PR description. Regenerating goldens to make a test pass is the one thing
  that gets a PR sent back without review.

- **The WASM size budget (`.wasm-size-budget`) is a reviewable number, not a formality.** Raising
  it is expected as converters land — but do it as a deliberate line in your diff with the reason
  stated, and check first that the growth is your format and not an unintended dependency
  (`cargo tree -p conv-wasm`).

- **CI's `js` job has no `.wasm` artifact.** It passes today only because `packages/engine` is
  wired to a `file:` dependency that exists after a local build. If you change how the engine
  consumes the wasm package, verify CI rather than assuming.

## Conventions

- **[Conventional Commits](https://www.conventionalcommits.org/):** `<type>(<scope>): <summary>`.
  Types: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `ci`, `build`. Scope is the
  package or crate — `feat(conv-core): add QOI encoder`.
- **SPDX header on new source files**, matching the directory's licence: `// SPDX-License-Identifier: MIT`
  for `crates/*` and `packages/*`, `// SPDX-License-Identifier: AGPL-3.0-only` for `apps/*`. Do not
  send a PR that only adds headers to existing files.
- **Public Rust items must be documented.** `#![warn(missing_docs)]` plus clippy's `-D warnings`
  makes this a build failure, not a style note.
- **`docs/adding-a-format.md` is kept in sync in the same PR as the code it describes.** If you
  change the `Converter` trait or the registry shape, update that doc in the same change.

## When you are unsure

- **Do not invent scope.** If the task implies work the repo has not committed to, say so rather
  than building it. CAD conversion was deliberately removed from this project's scope — do not
  re-add it to docs, catalogs, or the format catalog.
- **Do not weaken a gate to make it pass.** Clippy warnings, lint rules, the licence check and the
  size budget are all load-bearing. If a rule is genuinely wrong here, change the rule
  deliberately in its own commit with a reason — never `#[allow]` in passing or disable the job.
- **Prefer saying "this does not work yet".** The README's honesty about what is unshipped is a
  feature of this project, not an oversight to tidy up.

## Deeper docs

| Doc | When to read |
| --- | --- |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | System design, the two-runtime model, the media boundary |
| [`docs/adding-a-format.md`](docs/adding-a-format.md) | **Start here for any converter work** — full worked example |
| [`docs/BUILD.md`](docs/BUILD.md) | Toolchains, build order, troubleshooting |
| [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) | PR expectations, conventions in full |
| [`docs/SECURITY.md`](docs/SECURITY.md) | Why untrusted-input parsing makes bugs security issues |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | What is planned vs shipped |
