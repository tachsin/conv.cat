#!/usr/bin/env bash
#
# Licence boundary check.
#
# conv.cat is licensed in two halves (see ../../LICENSE):
#   MIT              crates/*, packages/*   — the reusable engine and libraries
#   AGPL-3.0-only    apps/*                 — the applications
#
# Dependencies must flow ONE WAY: apps depend on libraries, never the reverse.
# An MIT library that pulls in anything from apps/ becomes a derivative of AGPL
# code, which makes the MIT grant a promise the project cannot keep — and it
# still compiles, so nothing else catches it. See docs/ARCHITECTURE.md.
#
# Run locally before pushing:  ./.github/scripts/check-licence-boundary.sh
# Exits 0 when clean, 1 on the first violation found.

set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root" || exit 1

# Markdown is excluded on purpose: READMEs legitimately describe who consumes a
# package ("consumed by apps/web"), which is prose about the correct direction,
# not a dependency. Build output and vendored deps are excluded as noise.
prune=(
  ':(exclude)**/*.md'
  ':(exclude)**/node_modules/**'
  ':(exclude)**/dist/**'
  ':(exclude)**/target/**'
)

violations=0

report() {
  # $1 = human explanation, $2 = grep output (file:line:match)
  printf '\n\033[31m✗ %s\033[0m\n' "$1"
  printf '%s\n' "$2" | sed 's/^/    /'
  violations=$((violations + 1))
}

search() {
  # $1 = extended regex, $2... = pathspecs. Prints matches, empty if none.
  local pattern="$1"; shift
  # --untracked so a contributor's brand-new, not-yet-added file is checked too
  # (in CI everything is tracked, but locally this is the common case).
  git grep -n -I -E --untracked "$pattern" -- "$@" "${prune[@]}" 2>/dev/null
}

# 1. Rust: a path dependency reaching into apps/
if hits="$(search '^[[:space:]]*[^#]*path[[:space:]]*=[[:space:]]*"[^"]*apps/' 'crates/')"; then
  [ -n "$hits" ] && report \
    "A crate declares a path dependency on apps/ — an MIT crate cannot depend on AGPL code." \
    "$hits"
fi

# 2. JS manifests: the app packages named as a dependency of a library
if hits="$(search '@conv\.cat/(web|desktop)' 'packages/*/package.json' 'crates/')"; then
  [ -n "$hits" ] && report \
    "A library manifest references an app package — dependencies must flow apps → libraries only." \
    "$hits"
fi

# 3. Source: an import/require crossing the boundary, by package name or by path
if hits="$(search '(from|require|import)[^\n]*(@conv\.cat/(web|desktop)|\.\./apps/)' 'packages/' 'crates/')"; then
  [ -n "$hits" ] && report \
    "A library imports from an app — move the shared code down into conv-core / packages/data / packages/engine instead of reaching up." \
    "$hits"
fi

# 4. Anything else pointing into apps/ from the MIT half (test fixtures, config,
#    build scripts reading files out of an app directory).
if hits="$(search '(^|[^[:alnum:]_./-])apps/(web|desktop)' 'packages/' 'crates/')"; then
  # Filter out lines already reported by the more specific checks above.
  hits="$(printf '%s\n' "$hits" | grep -Ev '(from|require|import)[^\n]*\.\./apps/|path[[:space:]]*=' || true)"
  [ -n "$hits" ] && report \
    "A file in the MIT half references an app directory (fixture, config or script) — this couples MIT code to AGPL code." \
    "$hits"
fi

if [ "$violations" -gt 0 ]; then
  cat <<'EOF'

────────────────────────────────────────────────────────────────────────────
Licence boundary violated.

crates/* and packages/* are MIT so third parties can embed the conversion
engine in their own software. apps/* are AGPL-3.0-only. A library that
depends on an app is a derivative of AGPL code, and the MIT licence on it
becomes undeliverable — silently, because the code still builds.

Fix by moving the shared code DOWN, not by importing UP:
  conversion logic        → crates/conv-core
  catalogs / units / data → packages/data
  engine glue             → packages/engine
If it genuinely cannot move down, it is app-specific: duplicate it per app.

See docs/ARCHITECTURE.md § Dependency direction.
────────────────────────────────────────────────────────────────────────────
EOF
  exit 1
fi

printf '\033[32m✓ Licence boundary clean\033[0m — no crate or package depends on apps/.\n'
