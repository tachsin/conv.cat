#!/usr/bin/env bash
#
# WASM size budget check.
#
# crates/conv-wasm compiles to a .wasm payload that every visitor downloads before they can
# convert anything. Bundle size is therefore a product constraint, not a build detail — and the
# way it degrades is never a single bad commit, it is twenty merges each adding "just a bit".
#
# This script compares the RELEASE artifact against the number in .wasm-size-budget and fails
# when it is over. Raising the budget is fine; doing it accidentally is not. See the comments
# in .wasm-size-budget for what to check before you raise it.
#
# Run locally before pushing:
#   wasm-pack build crates/conv-wasm --target web --out-dir pkg --release
#   ./.github/scripts/check-wasm-size.sh
#
# Or let the build script do both:  ./scripts/build-all.sh --release --check
#
# Exits 0 when within budget, 1 when over, 2 when it cannot check (missing artifact/budget).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root" || exit 2

artifact="${1:-crates/conv-wasm/pkg/conv_wasm_bg.wasm}"
budget_file='.wasm-size-budget'

if [[ ! -f "$budget_file" ]]; then
  printf '✗ wasm size: %s is missing — cannot enforce a budget.\n' "$budget_file" >&2
  exit 2
fi

# The budget file is documented with a comment header; the budget is the last non-comment,
# non-empty line. Keeping the docs *in* the file is the point — the number is meaningless
# without the "raise it deliberately" instruction next to it.
budget="$(grep -vE '^[[:space:]]*(#|$)' "$budget_file" | tail -n 1 | tr -d '[:space:]')"

if ! [[ "$budget" =~ ^[0-9]+$ ]]; then
  printf '✗ wasm size: no valid byte count found in %s (read: %q)\n' "$budget_file" "$budget" >&2
  exit 2
fi

if [[ ! -f "$artifact" ]]; then
  printf '✗ wasm size: %s not found.\n' "$artifact" >&2
  printf '  Build it first: wasm-pack build crates/conv-wasm --target web --out-dir pkg --release\n' >&2
  exit 2
fi

actual="$(wc -c < "$artifact" | tr -d '[:space:]')"

# Integer-only KB rendering with one decimal, so the script stays dependency-free (no bc/python).
human() { printf '%d.%01d KB' $(( $1 / 1024 )) $(( ( $1 % 1024 ) * 10 / 1024 )); }

if (( actual > budget )); then
  over=$(( actual - budget ))
  printf '\n✗ WASM size budget exceeded\n\n' >&2
  printf '    artifact  %s\n' "$artifact" >&2
  printf '    actual    %s bytes (%s)\n' "$actual" "$(human "$actual")" >&2
  printf '    budget    %s bytes (%s)\n' "$budget" "$(human "$budget")" >&2
  printf '    over by   %s bytes (%s)\n\n' "$over" "$(human "$over")" >&2
  printf '  If this growth is intended, raise the number in %s in this PR and say why.\n' "$budget_file" >&2
  printf '  If it is not, check for an unintended dependency: cargo tree -p conv-wasm\n\n' >&2
  exit 1
fi

remaining=$(( budget - actual ))
printf '✓ WASM size %s bytes (%s) — within the %s byte budget, %s to spare.\n' \
  "$actual" "$(human "$actual")" "$budget" "$(human "$remaining")"
