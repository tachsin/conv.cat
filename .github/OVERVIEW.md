# .github

GitHub-specific configuration: CI workflows, issue templates, PR template. Nothing in here should
duplicate logic that belongs in the app/package/crate it targets; workflows call into each
workspace member's own scripts.

> **Why this file is `OVERVIEW.md` and not `README.md`** — every other workspace member documents
> itself in a `README.md`, but this directory must not. GitHub picks the README it renders on the
> repository landing page from `.github/`, then the root, then `docs/` — so a `.github/README.md`
> silently shadows the root [`README.md`](../README.md), and visitors would land on a page about
> CI config instead of the project. Do not rename this file back.

## CI

One workflow exists: `workflows/licence-boundary.yml`, which runs
`scripts/check-licence-boundary.sh` to enforce that no crate or package depends on `apps/*` —
the rule that keeps the MIT half of the repo free of AGPL code. It shipped ahead of the full CI
pipeline because the licence split is only a promise if something enforces it. The rest of CI
(Rust + JS + WASM build/test/lint gates on every PR) is wired up in a follow-up ticket — see
[`docs/ROADMAP.md`](../docs/ROADMAP.md). If that ticket folds this job into a larger workflow,
**keep the script** — the script is the contract, the workflow around it is disposable.

## Issue and PR templates

`ISSUE_TEMPLATE/` has three forms — bug report, format request, and converter contribution
(for proposing a new converter's design before implementing it) — plus `config.yml`, which
disables blank issues and points security reports and general questions elsewhere. See
[`docs/CONTRIBUTING.md`](../docs/CONTRIBUTING.md) for how contribution actually works, and
[`docs/adding-a-format.md`](../docs/adding-a-format.md) if you're here to add a conversion.

`PULL_REQUEST_TEMPLATE.md` is the PR checklist: tests, golden files, docs, i18n keys, licence
headers, and AI-assistance disclosure (see [`docs/ai-contributions.md`](../docs/ai-contributions.md)).
