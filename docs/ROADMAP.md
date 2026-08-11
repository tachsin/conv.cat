# Roadmap

## The vision

One tool for every conversion, everywhere: images, video, audio, text and data, units,
timezones, and CAD, all converted locally, all powered by one Rust engine, available as a web
app, an installable PWA, a native desktop app, and eventually a CLI — without maintaining four
separate implementations of "how do I convert a file."

This is a from-scratch rebuild. It is staged deliberately small at first (see
[Phase 2](#phase-2--web-mvp)) rather than trying to match the old app's surface area on day one.

## Where this repo stands relative to the live product

**conv.cat, the product you can visit today, does not yet run on this repository.** It runs on a
separate, pre-OSS codebase with ~5,200 indexed pages, 862 per-pair landing pages, and 6 live
locales. This repo is the public rebuild, starting from a clean architecture (see
[ARCHITECTURE.md](ARCHITECTURE.md)). Whether and how it takes over the live domain is a
deliberate cutover decision — protecting years of search ranking is treated as seriously as any
engineering milestone here — tracked in the project backlog and not yet finalized. Until that
decision lands, treat this repo as a rebuild-in-progress, not as the thing currently running in
production.

## Status legend

| | |
| --- | --- |
| ✅ | Shipped, on `main` |
| 🚧 | Actively being built |
| 📋 | Scoped — a backlog ticket exists |
| 💭 | Vision-stage — direction is set, not yet scoped into a ticket |

## Phase 0 — Foundation

| Status | Item |
| --- | --- |
| ✅ | Monorepo scaffold — pnpm + Cargo workspaces, every member buildable independently |
| ✅ | Split licence — MIT for `crates/*`/`packages/*`, AGPL-3.0-only for `apps/*` |
| ✅ | Licence-boundary check enforced in CI on every push and PR |
| ✅ | Community docs set — this file, [README](../README.md), [CONTRIBUTING](CONTRIBUTING.md), [ARCHITECTURE](ARCHITECTURE.md), [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md), [SECURITY](SECURITY.md), [adding-a-format](adding-a-format.md), [ai-contributions](ai-contributions.md) |
| 📋 | Full CI — Rust + JS + WASM build/test/lint gates on every PR (today only the licence-boundary check runs) |

## Phase 1 — The engine spine

Nothing converts anything yet. This phase is the architectural foundation every format and every
app depends on — deliberately built and reviewed before ten converters are written against it.

| Status | Item |
| --- | --- |
| 📋 | `conv-core` foundation — the `Converter` trait, the `Format` registry, the typed error enum, the progress/cancellation hook |
| 📋 | Golden-file conformance suite — the correctness oracle that makes a stranger's format PR reviewable at all |
| 📋 | `conv-wasm` bindings + npm build pipeline, `packages/engine`'s WASM/native runtime selection, the Web Worker path |

## Phase 2 — Web MVP

A single-page converter: drag a file in, pick a target format, convert with visible progress,
download. English only. No per-pair landing pages at launch — that was a legacy SEO strategy that
produced a documented duplicate-content problem, and bringing it back (if it comes back at all)
is a deliberate decision, not a default.

| Status | Item |
| --- | --- |
| 📋 | First vertical slice: **units**, end to end (Rust → WASM → web) — chosen because it's pure computation with no binary parsing, so a pipeline bug and a domain bug can't be confused for each other. Includes the niche categories worth keeping: clothing sizes, cooking measurements, and yes, cat/dog years. |
| 📋 | `apps/web` — the single-page converter shell, with the "nothing is uploaded" proof made visible, not just claimed |
| 📋 | Image conversion ported to `conv-core` |

## Phase 3 — Full format parity + i18n

| Status | Item |
| --- | --- |
| 📋 | Video/audio via ffmpeg-wasm behind `packages/engine`, in `packages/media` (see [the media boundary](ARCHITECTURE.md#the-media-boundary-video-and-audio-stay-on-ffmpeg-wasm) — this one deliberately does not move to Rust) |
| 📋 | Text/data and CAD conversion ported to `conv-core` |
| 📋 | i18n re-architecture — all 6 locales (`en`, `de`, `el`, `es`, `fr`, `tr`) back, this time with a CI key-drift guardrail and placeholder validation from day one, and an evaluation of a translation platform (Weblate/Crowdin) so contributing a translation doesn't require cloning a monorepo |

## Phase 4 — PWA

💭 Installable, offline-capable web app — the WASM engine and the format catalog already run
entirely client-side, so this is primarily manifest/service-worker/caching work on top of
Phase 2–3, not new conversion logic. Not yet broken into tickets.

## Phase 5 — Desktop

📋 `apps/desktop` — a real, cross-platform Tauri app on the native `conv-core` bindings, no WASM
round-trip. **Not implemented today** — `apps/desktop` is currently a scaffold with a placeholder
entry point (see its README). A ticket for the real app exists in the backlog; it has not been
started.

## Phase 6 — CLI

💭 A thin binary over `conv-core` for scripting and automation — batch-convert a directory,
pipe-friendly, no browser or Tauri involved. This is a natural consequence of having one
framework-free engine crate, but it is not yet scoped into a ticket.

## Format catalog — target state

Status reflects the engine, not the UI: "Planned" means no `crates/conv-core` implementation
exists yet, regardless of what the legacy site currently offers on the live domain.

| Category | Target formats | Status |
| --- | --- | --- |
| Units | Length, mass, temperature, volume, clothing sizes, cooking measurements, cat/dog years, and more | 📋 Planned — first vertical slice, Phase 2 |
| Images | PNG, JPEG, WebP, AVIF, BMP, GIF, ICO, QOI, HEIC (decode) | 📋 Planned — Phase 2 |
| Text & Data | CSV, JSON, HTML, Markdown | 📋 Planned — Phase 3 |
| Video & Audio | Whatever ffmpeg-wasm supports, via `packages/media` | 📋 Planned — Phase 3 |
| Timezones | IANA zones, interactive world map | 📋 Planned — Phase 3 |
| CAD | STL, STEP, OBJ | 📋 Planned — Phase 3 |

See [docs/adding-a-format.md](adding-a-format.md) if you want to pull one of these forward
yourself, or add one that isn't listed.

## Out of scope

- **Wrapping ffmpeg in Rust.** See [ARCHITECTURE.md § The media boundary](ARCHITECTURE.md#the-media-boundary-video-and-audio-stay-on-ffmpeg-wasm).
- **Server-side conversion.** The privacy claim ("your files never leave your device") is the
  whole point — a server upload path would contradict it. See the [README](../README.md).
- **Bringing back 862 mail-merged landing pages.** If per-pair SEO pages return, they return
  fewer and genuinely differentiated, as a deliberate decision, not by default.

## Influence the roadmap

Open a [GitHub Discussion](https://github.com/tachsin/conv.cat/discussions) or an issue using the
format-request template. Ticket priority in this doc reflects current planning, not a promise —
see [CONTRIBUTING.md](CONTRIBUTING.md) for how work actually gets picked up.
