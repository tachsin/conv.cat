#!/usr/bin/env bash
#
# build-all — build every artifact in the conv.cat monorepo, in dependency order.
#
# conv.cat is two build systems sharing one repository: a pnpm workspace
# (apps/*, packages/*) and a Cargo workspace (crates/*), bridged by a
# WebAssembly artifact. Building "the project" is therefore four separate
# toolchain invocations whose ORDER MATTERS:
#
#     crates/*  ──cargo──▶  conv-wasm  ──wasm-pack──▶  packages/*  ──▶  apps/*
#
# The Rust engine builds first because the WASM artifact is compiled from it;
# the WASM artifact builds before the JS packages because packages/engine
# consumes it; the apps build last because they sit on top of the packages.
# Get that order wrong and you either build against a stale .wasm or fail on a
# missing import — which is exactly why this script exists instead of a note in
# the README telling everyone to run four commands by hand.
#
# Usage:
#   ./scripts/build-all.sh              debug build of everything
#   ./scripts/build-all.sh --release    optimised build (slower, what you ship)
#   ./scripts/build-all.sh --check      also run fmt, clippy, tests, lint, licence boundary
#                                       (with --release, also the WASM size budget)
#   ./scripts/build-all.sh --clean      remove build outputs first, then build
#   ./scripts/build-all.sh --help       this message
#
# Flags combine: `--release --check` is the full pre-release gate.
#
# See docs/BUILD.md for what each step produces, how to build individual pieces,
# and how to troubleshoot the failures that actually happen.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# ─── options ────────────────────────────────────────────────────────────────

release=0
run_checks=0
do_clean=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) release=1 ;;
    --check)   run_checks=1 ;;
    --clean)   do_clean=1 ;;
    -h|--help)
      sed -n '3,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      printf 'build-all: unknown option %s\n\n' "$1" >&2
      printf 'Run with --help for usage.\n' >&2
      exit 2
      ;;
  esac
  shift
done

# ─── output helpers ─────────────────────────────────────────────────────────
# Colour only when attached to a terminal, so CI logs stay clean.

if [[ -t 1 ]]; then
  bold=$'\033[1m'; dim=$'\033[2m'; red=$'\033[31m'; green=$'\033[32m'; reset=$'\033[0m'
else
  bold=''; dim=''; red=''; green=''; reset=''
fi

step_n=0
step() {
  step_n=$((step_n + 1))
  printf '\n%s▶ [%d] %s%s\n' "$bold" "$step_n" "$1" "$reset"
}
info() { printf '%s  %s%s\n' "$dim" "$1" "$reset"; }
die()  { printf '\n%s✗ build-all failed: %s%s\n' "$red" "$1" "$reset" >&2; exit 1; }

start_time=$SECONDS

# ─── prerequisites ──────────────────────────────────────────────────────────
# Fail here with an actionable message rather than 40 lines into a build with a
# confusing toolchain error.

step 'Checking prerequisites'

missing=()
for tool in pnpm node cargo; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done

# wasm-pack is only needed for the WASM step, but check it up front so the run
# doesn't die three minutes in.
command -v wasm-pack >/dev/null 2>&1 || missing+=("wasm-pack")

if [[ ${#missing[@]} -gt 0 ]]; then
  printf '\n%s✗ Missing required tools: %s%s\n\n' "$red" "${missing[*]}" "$reset" >&2
  cat >&2 <<'EOF'
Install them with:
  pnpm       npm install -g pnpm@11      (or: corepack enable)
  node       nvm install                 (version comes from .nvmrc)
  cargo      https://rustup.rs           (toolchain comes from rust-toolchain.toml)
  wasm-pack  cargo install wasm-pack --locked --version "$(cat .wasm-pack-version)"

See docs/BUILD.md for the full prerequisites section.
EOF
  exit 1
fi

# wasm-pack is the one toolchain the repo cannot pin declaratively — there is no
# .nvmrc/rust-toolchain.toml equivalent, so .wasm-pack-version plus this check is
# the pin. It matters more than a normal version nag: wasm-pack bundles a
# wasm-bindgen CLI that must match the wasm-bindgen CRATE version in
# crates/conv-wasm/Cargo.toml, and a mismatch surfaces as a confusing runtime
# error in the browser rather than a build failure.
#
# Local drift warns (so a contributor is never blocked by a version bump);
# CI fails, because that is where a reproducible artifact actually matters.
# Same split as --frozen-lockfile above.
if [[ -f .wasm-pack-version ]]; then
  wasm_pack_pinned="$(tr -d '[:space:]' < .wasm-pack-version)"
  wasm_pack_actual="$(wasm-pack --version | cut -d' ' -f2)"
  if [[ "$wasm_pack_actual" != "$wasm_pack_pinned" ]]; then
    msg="wasm-pack $wasm_pack_actual does not match the pinned $wasm_pack_pinned"
    fix="cargo install wasm-pack --locked --version $wasm_pack_pinned"
    if [[ "${CI:-}" == 'true' ]]; then
      printf '\n%s✗ %s%s\n  Fix: %s\n' "$red" "$msg" "$reset" "$fix" >&2
      exit 1
    fi
    printf '%s  ! %s%s\n    Fix: %s\n' "$red" "$msg" "$reset" "$fix"
    printf '%s    Continuing — if the browser engine misbehaves, this is the first thing to rule out.%s\n' \
      "$dim" "$reset"
  fi
fi

# rustup reads rust-toolchain.toml itself, but the wasm target is a separate
# component that a fresh install will not have.
if command -v rustup >/dev/null 2>&1; then
  if ! rustup target list --installed 2>/dev/null | grep -qx 'wasm32-unknown-unknown'; then
    info 'Adding missing wasm32-unknown-unknown target…'
    rustup target add wasm32-unknown-unknown
  fi
fi

info "node      $(node --version)"
info "pnpm      $(pnpm --version)"
info "cargo     $(cargo --version | cut -d' ' -f2)"
info "wasm-pack $(wasm-pack --version | cut -d' ' -f2)"

# ─── clean ──────────────────────────────────────────────────────────────────

if [[ $do_clean -eq 1 ]]; then
  step 'Cleaning build outputs'
  pnpm -r --if-present run clean || die 'pnpm clean failed'
  cargo clean || die 'cargo clean failed'
  rm -rf crates/conv-wasm/pkg
  info 'Removed dist/, target/ and crates/conv-wasm/pkg'
fi

# ─── 1. JS dependencies ─────────────────────────────────────────────────────

step 'Building Rust crates'
cargo_flags=()
[[ $release -eq 1 ]] && cargo_flags+=(--release)
cargo build --workspace "${cargo_flags[@]}" || die 'cargo build failed'

# ─── 2. WebAssembly artifact ────────────────────────────────────────────────

step 'Building WebAssembly artifact (conv-wasm)'
# --target web produces an ES module loadable directly by the browser and by
# bundlers, which is what packages/engine expects. --out-dir is relative to the
# crate, giving crates/conv-wasm/pkg.
wasm_flags=(--target web --out-dir pkg)
if [[ $release -eq 1 ]]; then
  wasm_flags+=(--release)
else
  wasm_flags+=(--dev)
fi
wasm-pack build crates/conv-wasm "${wasm_flags[@]}" || die 'wasm-pack build failed'

if [[ -f crates/conv-wasm/pkg/conv_wasm_bg.wasm ]]; then
  wasm_size="$(wc -c < crates/conv-wasm/pkg/conv_wasm_bg.wasm | tr -d ' ')"
  info "conv_wasm_bg.wasm — $((wasm_size / 1024)) KB"
  # The WASM payload ships to every visitor. Surfacing the number on every
  # build is how a regression gets noticed before CI enforces a budget.
fi

# ─── 3. JS dependencies ─────────────────────────────────────────────────────
# Deliberately AFTER the WASM build, not before: packages/engine has a real `file:` dependency
# on crates/conv-wasm/pkg (see its package.json), so `pnpm install` has nothing to link against
# until step 2 has produced that directory. Installing first was the original order here and
# broke silently the moment that dependency became real — see docs/BUILD.md's build-order
# section and packages/engine/README.md if you're wondering why this looks backwards from a
# "usually you install JS deps first" instinct.

step 'Installing JS dependencies'
# --frozen-lockfile in CI so a drifting lockfile fails the build instead of
# being silently "fixed"; locally, allow the lockfile to update.
if [[ "${CI:-}" == 'true' ]]; then
  pnpm install --frozen-lockfile || die 'pnpm install failed (lockfile out of date?)'
else
  pnpm install || die 'pnpm install failed'
fi

# ─── 4. JS packages and apps ────────────────────────────────────────────────

step 'Building JS packages and apps'
# pnpm -r walks the workspace in topological order, so packages/* build before
# the apps that depend on them.
pnpm -r --if-present run build || die 'pnpm build failed'

# ─── 5. optional checks ─────────────────────────────────────────────────────

if [[ $run_checks -eq 1 ]]; then
  step 'Checking formatting (cargo fmt)'
  cargo fmt --all --check || die 'cargo fmt found unformatted code — run: cargo fmt --all'

  step 'Linting Rust (clippy)'
  cargo clippy --workspace --all-targets -- -D warnings || die 'clippy found issues'

  step 'Running Rust tests'
  cargo test --workspace "${cargo_flags[@]}" || die 'cargo test failed'

  step 'Linting JS'
  # Runs from the repo root: one flat config (eslint.config.js) covers the whole workspace.
  pnpm lint || die 'eslint found issues — run: pnpm lint:fix'

  step 'Typechecking JS'
  pnpm -r --if-present run typecheck || die 'typecheck failed'

  # Only meaningful against a release artifact — a debug .wasm is several times larger, so
  # measuring it would either fail constantly or force a budget so loose it catches nothing.
  # CI always builds --release, so the budget is enforced there on every PR regardless.
  if [[ $release -eq 1 ]]; then
    step 'Checking WASM size budget'
    ./.github/scripts/check-wasm-size.sh || die 'WASM size budget exceeded'
  fi

  step 'Checking licence boundary'
  # Enforces that no MIT crate/package depends on an AGPL app. See
  # .github/scripts/check-licence-boundary.sh for why this is load-bearing.
  ./.github/scripts/check-licence-boundary.sh || die 'licence boundary violated'
fi

# ─── done ───────────────────────────────────────────────────────────────────

elapsed=$((SECONDS - start_time))
printf '\n%s✓ Build complete in %dm %02ds%s' "$green" $((elapsed / 60)) $((elapsed % 60)) "$reset"
[[ $release -eq 1 ]]   && printf ' %s(release)%s' "$dim" "$reset"
[[ $run_checks -eq 1 ]] && printf ' %s(checks passed)%s' "$dim" "$reset"
printf '\n'

if [[ $run_checks -eq 0 ]]; then
  printf '%sRun with --check to also verify formatting, lints, tests and the licence boundary.%s\n' \
    "$dim" "$reset"
fi
