# docs

Community and architecture documentation for conv.cat.

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — the Rust-core-to-WASM/native design, the dependency
  boundary between the MIT libraries and the AGPL apps, the media boundary, and how a conversion
  request actually flows.
- [`BUILD.md`](BUILD.md) — prerequisites, the `build-all` script, the Rust → WASM → JS build
  order and why it matters, per-piece commands, and troubleshooting.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — setup, the monorepo tour, commit conventions, how to run
  tests, PR expectations.
- [`adding-a-format.md`](adding-a-format.md) — the complete worked example for adding one
  conversion end to end. Read this before opening a converter PR.
- [`ROADMAP.md`](ROADMAP.md) — the staged plan from the current scaffold to web → PWA → desktop →
  CLI, and where this repo stands relative to the live product.
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) — Contributor Covenant 2.1.
- [`SECURITY.md`](SECURITY.md) — private vulnerability disclosure; why malformed-input crashes and
  WASM memory-safety bugs are treated as security reports here.
- [`ai-contributions.md`](ai-contributions.md) — the policy on AI-assisted PRs: welcome, but the
  submitter is accountable, must have tested it, and must disclose it.

This folder is documentation only — it must never contain code.
