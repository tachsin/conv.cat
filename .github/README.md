# .github

GitHub-specific configuration: CI workflows, issue templates, PR template, `CODEOWNERS`. This
folder is mostly a placeholder — the real CI (Rust + JS + WASM gates on every PR) is wired up in
a follow-up ticket ("CI for the new repo"). Nothing in here should duplicate logic that belongs
in the app/package/crate it targets; workflows call into each workspace member's own scripts.

One workflow already exists: `workflows/licence-boundary.yml`, which runs
`scripts/check-licence-boundary.sh` to enforce that no crate or package depends on `apps/*` —
the rule that keeps the MIT half of the repo free of AGPL code. It shipped ahead of the CI
ticket because the licence split is only a promise if something enforces it. The full pipeline
may fold this job into a larger workflow; if it does, **keep the script** — the script is the
contract, the workflow around it is disposable.
