# Security Policy

## Why this matters more than usual for a converter

Every converter in this project — image, text/data, CAD, and via `packages/media`, video/audio —
parses **untrusted binary files**. A user drops in a file they downloaded from anywhere, and one
of `crates/conv-core`, `crates/conv-wasm`, or ffmpeg-wasm parses it. That makes two categories of
bug real security issues here, not just correctness bugs:

- **Malformed-input crashes.** A panic in `crates/conv-core` reachable from user-supplied bytes
  (out-of-bounds slice access, an integer overflow in a size calculation, an unhandled malformed
  header) is a denial-of-service at minimum. Per the project's own design rule (see
  [ARCHITECTURE.md](ARCHITECTURE.md#the-conversion-request-lifecycle)), every parser must return
  a typed `ConvertError` on malformed input, never panic. If you find a crafted input that panics
  a converter, that's a valid report even if it "just" crashes a tab.
- **WASM memory-safety issues.** `crates/conv-wasm` is the boundary between untrusted file bytes
  and the browser's WASM linear memory. A bug that reads or writes outside an intended buffer
  here — even one that "only" corrupts adjacent WASM memory rather than escaping the sandbox — is
  a security report, not a crash report.

We also welcome reports of any way a crafted file could exfiltrate data, execute unintended code,
or defeat the local-only conversion model (see the root [README](../README.md)) — for example, a
converter that, under some condition, makes a network request it shouldn't.

## What is out of scope

Since conversions run entirely client-side (no upload, no server-side processing of the files
you convert — that's the whole point of this project), the typical server-side attack surface
mostly doesn't apply here. Reports about `apps/web`'s own hosting infrastructure, third-party
dependency CVEs with no demonstrated exploit path in our usage, or missing-format feature
requests are better filed as regular issues, not security reports — see
[CONTRIBUTING.md](CONTRIBUTING.md) for the format-request template.

## Reporting a vulnerability

**Please do not open a public GitHub issue for a security report.**

Preferred: use
[GitHub Security Advisories](https://github.com/tachsin/conv.cat/security/advisories/new) for
this repository — it's private by default and lets us coordinate a fix before disclosure.

If you'd rather not use GitHub, email **tachsinatalay@gmail.com** with a description of the issue,
a reproduction case if you have one (a crafted input file is ideal — see the note on fixture
licensing below), and the affected package/crate.

Please include, where relevant:

- The crate or package affected (`conv-core`, `conv-wasm`, `packages/media`, `apps/web`, ...).
- A minimal reproduction — ideally a small crafted file that triggers the issue. If you can share
  the file, please make sure it's something you're comfortable being redistributed as a test
  fixture (self-generated or otherwise unencumbered) — it may end up in the malformed-input
  corpus described in [ARCHITECTURE.md § The conformance suite](ARCHITECTURE.md#the-conformance-suite)
  once fixed, crediting you if you'd like.
- What you expected to happen versus what happened (crash, panic, memory corruption, unexpected
  network activity, etc.).

## What to expect

This is an early-stage, largely volunteer-maintained project — response times are best-effort,
not an SLA. We aim to acknowledge a report within 5 business days and to follow up with at least
a triage assessment (confirmed / not reproducible / not in scope) shortly after. Fixes are
prioritized by severity: a panic reachable from a crafted file that any converter user could hit
is treated as high priority.

We'll credit reporters in the fix's changelog/release notes unless you'd prefer to stay anonymous
— let us know your preference when you report.

## Supported versions

conv.cat is pre-1.0 and ships from a single rolling `main` branch — there is no maintained
older-version branch yet. Security fixes land on `main`. This section will be expanded once
tagged releases exist.
