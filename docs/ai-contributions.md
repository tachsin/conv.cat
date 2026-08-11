# AI-assisted contributions

Let's be upfront about something: a meaningful share of this codebase, including this document,
is written with AI coding agents. That's not a secret and it's not a problem by itself — it's
just honest to say, because it shapes the policy below. If a project built this way pretends
otherwise, contributors reasonably assume a higher bar of human authorship than actually exists,
and reviewers waste time re-litigating it PR by PR instead of having one clear rule.

## AI-assisted PRs are welcome

Use whatever tools help you — an AI pair programmer, a code-generation agent, a translation
model for i18n strings, whatever. The tool doesn't matter. What matters is what you did with the
output before you opened the PR.

## The rules

1. **You are accountable for the code, not the tool.** "The agent wrote it" is not a defense for
   a bug, a licence violation, or a security issue. If your name is on the PR, you're vouching for
   what's in it, the same as if you'd typed every character yourself.

2. **You must have run it and tested it.** Not "it looks right" — actually run
   `cargo test` / `cargo clippy -- -D warnings` / `pnpm typecheck` locally, and for a new or
   changed converter, actually run the golden-file suite (see
   [adding-a-format.md](adding-a-format.md)) and confirm it passes for the reason you think it
   passes. A generated converter that happens to satisfy a golden file for the wrong reason (an
   edge case the fixture doesn't cover) is worse than no converter — it looks done.

3. **Disclose the assistance in the PR description.** A one-line note is enough: "Implementation
   drafted with \[tool], reviewed and tested locally" or similar. This isn't a confession, it's
   context that helps the reviewer calibrate what to look at closely — generated code has
   characteristic failure modes (plausible-looking but wrong error handling, subtly incorrect
   edge cases, invented API calls) that are worth a second look regardless of how good the tool
   is.

4. **Unreviewed generated PRs get closed, not iterated on.** If a PR reads like raw agent output
   dropped in without the submitter having exercised it — no evidence of local testing, a
   description that's clearly just the tool's own summary, golden fixtures that look
   auto-generated to satisfy the test rather than to prove correctness — a maintainer will close
   it rather than spend review cycles debugging code nobody has actually run. This isn't
   punitive; it's the only sustainable policy for a public conversion library that anyone can
   point an agent at and ask it to "add a format." Without this rule, the project's actual
   maintenance cost is an unbounded queue of plausible-looking, untested converters — exactly the
   flood a project like this is uniquely exposed to, since "add a format" is such a well-scoped,
   agent-friendly task.

## Why format contributions specifically need this

`crates/conv-core` converters parse untrusted binary input (see [SECURITY.md](SECURITY.md)) and
ship to every user of the web app and the desktop app. A confidently-wrong generated converter —
one that mishandles a malformed file, or silently produces incorrect output on an edge case the
author never tried — is a correctness and security regression shipped under someone's name. The
[golden-file conformance suite](ARCHITECTURE.md#the-conformance-suite) exists precisely to catch
this class of problem, but it only works if the fixtures were chosen to actually probe the
format, not generated to make a specific implementation pass.

## What this is not

This is not an anti-AI policy. The project itself wouldn't exist in its current form without
AI-assisted work, and that's fine. It's a policy against *unreviewed* output — human or AI — being
merged into a codebase that other people's software depends on for correct file conversion.
