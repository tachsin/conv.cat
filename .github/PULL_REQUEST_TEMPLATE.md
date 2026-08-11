## What does this PR do?

<!-- One or two sentences. Link an issue if there is one. -->

## Type of change

- [ ] New or updated converter / format
- [ ] Bug fix
- [ ] Docs
- [ ] Chore / infra / CI
- [ ] Other:

## AI assistance

<!--
This project welcomes AI-assisted contributions — see docs/ai-contributions.md. Please disclose
briefly, e.g. "Drafted with [tool], reviewed and tested locally" or "None, written by hand."
-->

## Checklist

- [ ] `cargo test` passes locally (and `cargo test -p conv-core` if you touched conversion logic)
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] `pnpm typecheck` passes
- [ ] `./.github/scripts/check-licence-boundary.sh` passes (if you touched `crates/*` or `packages/*`)
- [ ] **Golden files added or updated**, if this adds/changes a converter — see
      [docs/adding-a-format.md](../docs/adding-a-format.md#step-4--golden-file-tests)
      (n/a if this PR doesn't touch a converter)
- [ ] If a golden file was *regenerated* rather than added, I explained the diff above — see
      [CONTRIBUTING.md](../docs/CONTRIBUTING.md#running-tests)
- [ ] **Docs updated** — README / ARCHITECTURE / ROADMAP / adding-a-format.md, whichever this
      touches (n/a if none apply)
- [ ] **i18n key(s) added** to at least `en.json`, if this adds anything user-visible — see
      [docs/adding-a-format.md § i18n](../docs/adding-a-format.md#step-6--add-the-i18n-keys)
      (n/a if this PR has no user-visible strings)
- [ ] **Licence header correct** — new source files carry the SPDX identifier matching their
      directory's licence (see [CONTRIBUTING.md § Licence headers](../docs/CONTRIBUTING.md#licence-headers))
- [ ] I have not added anything to `crates/*` or `packages/*` that imports from `apps/*`

## Anything a reviewer should know?

<!-- Design tradeoffs, things you're unsure about, follow-up work you deliberately left out. -->
