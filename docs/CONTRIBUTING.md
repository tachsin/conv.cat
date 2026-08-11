# Contributing to conv.cat

Thanks for the interest. This doc covers setup, the monorepo layout, conventions, and what a PR
needs before it can merge. If you specifically want to add a conversion format, read
[`docs/adding-a-format.md`](adding-a-format.md) instead — it's the complete worked example and
will get you there faster than this file.

This project is largely built with AI-assisted agents, and that's fine — see
[`docs/ai-contributions.md`](ai-contributions.md) for the rules that come with that (short
version: you're accountable for what you submit, and you must have actually run it). If you are
pointing a coding agent at this repo, [`AGENTS.md`](../AGENTS.md) in the root is written for the
agent itself — the invariants, the commands, and the traps that look like bugs but aren't.

## Setup

Toolchains are pinned so `nvm`/`rustup` pick the right version automatically — you shouldn't need
to hunt for a compatible version yourself.

```bash
git clone https://github.com/tachsin/conv.cat.git
cd conv.cat

nvm use                        # reads .nvmrc → Node 26.3.0
                               # rustup reads rust-toolchain.toml automatically

./scripts/build-all.sh         # installs deps and builds everything, in order
```

`build-all.sh` verifies your toolchain before it starts and tells you what's missing, so it's
also the fastest way to find out whether your environment is ready. Run
`./scripts/build-all.sh --check` before pushing — it runs the same gates as CI (fmt, clippy,
tests, typecheck, licence boundary).

If you don't have `pnpm`, `rustup` or `wasm-pack` yet: `corepack enable` gets you the pinned
`pnpm` from `packageManager` in the root `package.json`; [rustup.rs](https://rustup.rs) gets you
`rustup`; and `cargo install wasm-pack --locked --version "$(cat .wasm-pack-version)"` gets you
the last one at the version this repo pins.

Build order, per-piece commands for the inner dev loop, and troubleshooting live in
[`BUILD.md`](BUILD.md).

Requirements: Node ≥ 26, pnpm 11.21 (via corepack), Rust 1.96 (via rustup). No Docker, no other
services — everything here runs entirely client-side by design (see the root
[README](../README.md)), and that includes the dev environment.

## Monorepo tour

```
conv.cat/
├─ apps/
│  ├─ web/          Next.js web app                    AGPL-3.0-only
│  └─ desktop/       Tauri desktop app                   AGPL-3.0-only
├─ crates/
│  ├─ conv-core/    the conversion engine, plain Rust    MIT
│  └─ conv-wasm/    wasm-bindgen bindings over conv-core  MIT
├─ packages/
│  ├─ engine/       TS wrapper: WASM (web) / native (desktop)  MIT
│  ├─ media/        video/audio via ffmpeg-wasm            MIT
│  └─ data/         format catalogs, units, timezones, i18n MIT
├─ docs/            you are here
└─ .github/         CI workflows, issue/PR templates
```

Every directory has its own `README.md` stating what belongs there and what must never end up
there — read the relevant one before touching that code. The one rule that spans all of them:
**conversion logic lives in `crates/conv-core`, nowhere else.** Not in the apps, not in
`packages/data`, not duplicated in TypeScript "just for now." See
[`docs/ARCHITECTURE.md`](ARCHITECTURE.md) for why, and for the dependency-direction rule
(`apps/*` may depend on `crates/*`/`packages/*`, never the reverse) that's enforced in CI.

## Branch and commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/). Format:

```
<type>(<scope>): <short description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`. Scope is usually the package or
crate you touched — `feat(conv-core): add QOI encoder`, `fix(apps-web): drop-zone keyboard focus`,
`docs(adding-a-format): fix registry snippet`. PR titles should follow the same convention; they
often become the squash-merge commit message.

Branch names aren't strictly enforced, but `type/short-description` (e.g.
`feat/qoi-encoder`) keeps things readable in the branch list.

## Running tests

```bash
# Rust — this is where the real logic lives, and where the conformance suite runs
cargo test

# Per-crate, if you only touched one
cargo test -p conv-core
cargo clippy -- -D warnings

# JS/TS — lint and typecheck stand in for a test suite for now; there is no JS test
# runner wired up yet (tracked in the backlog, tagged CI)
pnpm lint
pnpm typecheck
pnpm build

# Licence boundary — no crate or package may depend on apps/*, checked on every push
./.github/scripts/check-licence-boundary.sh
```

If you're adding or touching a converter, `cargo test -p conv-core` must include the golden-file
suite for it — see [`docs/adding-a-format.md`](adding-a-format.md#step-4--golden-file-tests). This
is enforced in CI (`.github/workflows/ci.yml`, job **Rust — fmt, clippy, test**, runs `cargo fmt
--check`, `cargo clippy -- -D warnings`, and `cargo test --workspace` on every push and PR), not
just documented here — a conformance regression fails the build.

**Regenerating a golden file is not a routine fix.** If your change legitimately changes a
converter's output (a bug fix, an upstream codec update), regenerate it with:

```bash
UPDATE_GOLDENS=1 cargo test -p conv-core --test golden
```

then `git diff` the result and explain the diff in your PR description — what changed and why the
new output is correct. A PR that silently regenerates goldens to make a test pass is the one case
that gets sent back without review.

## PR expectations

- Keep PRs small and focused — one format, one bug, one doc, not a grab-bag.
- Fill in the PR template checklist honestly; an unchecked box with no explanation is a request
  for changes, not an oversight to ignore.
- New or changed converters need golden fixtures (a valid case and a malformed-input case) — see
  [adding-a-format.md](adding-a-format.md).
- If you touched anything a user sees, update the relevant i18n key in `en.json` at minimum (see
  [adding-a-format.md § i18n](adding-a-format.md#step-6--add-the-i18n-keys)).
- Run `./.github/scripts/check-licence-boundary.sh` before pushing if you touched
  `crates/*`/`packages/*` — it's fast and it's what CI blocks on.
- Disclose AI assistance per [ai-contributions.md](ai-contributions.md) if it applies. This isn't
  a formality: unreviewed generated PRs get closed without much back-and-forth, because reviewing
  a plausible-looking but subtly wrong converter costs more than writing it.

### Licence headers

New source files should start with an SPDX identifier matching their directory's licence (see the
table in [ARCHITECTURE.md](ARCHITECTURE.md#dependency-direction)):

```rust
// SPDX-License-Identifier: MIT
```

```rust
// SPDX-License-Identifier: AGPL-3.0-only
```

Existing scaffold files predate this convention — add the header when you next touch a file that
doesn't have one, rather than sending a PR that only adds headers.

## Where to ask questions

- **Questions about a specific area** — open a
  [GitHub Discussion](https://github.com/tachsin/conv.cat/discussions) or comment on the relevant
  issue.
- **Found a bug** — open an issue with the bug report template.
- **Want to work on something not tracked yet** — open an issue with the format-request or
  converter-contribution template before writing a lot of code, especially for a new format
  category; it avoids two people porting the same thing, and lets a maintainer flag design
  concerns (registry shape, option types) before you're deep into an implementation.
- **Security issue** — do **not** open a public issue. See [SECURITY.md](SECURITY.md).

## Licence

By contributing, you agree your contribution is licensed under the licence that already applies
to the directory you're contributing to — MIT for `crates/*`/`packages/*`, AGPL-3.0-only for
`apps/*`. See the root [LICENSE](../LICENSE) for the full split and the
[Trademark](../README.md#trademark) note on the conv.cat name and logo.
