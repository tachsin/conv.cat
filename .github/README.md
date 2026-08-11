# .github

GitHub-specific configuration: CI workflows, issue templates, PR template, `CODEOWNERS`. This
folder is a placeholder — the real CI (Rust + JS + WASM gates on every PR) is wired up in a
follow-up ticket ("CI for the new repo"). Nothing in here should duplicate logic that belongs
in the app/package/crate it targets; workflows call into each workspace member's own scripts.
