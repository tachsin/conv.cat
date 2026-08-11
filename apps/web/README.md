# @conv.cat/web

The Next.js web app — the public conv.cat single-page converter shell. License: **AGPL-3.0-only**.

This package is UI only: pages, components, routing, styling, client-side wiring to
`@conv.cat/engine`. It must never contain conversion logic, format-specific parsing/encoding,
or unit/catalog data of its own — that all lives in `crates/conv-core` (via `@conv.cat/engine`)
and `packages/data`. If you find yourself writing a converter, encoder, or format catalog
in here, it belongs somewhere else in the workspace instead.

This is currently a scaffold: no Next.js app has been wired up yet. See the backlog for the
ticket that builds the real app shell.
