# @conv.cat/media

Video and audio conversion via ffmpeg-wasm. License: **MIT**.

This package stays TypeScript on purpose — it does not get ported to `crates/conv-core`.
ffmpeg-wasm is already a battle-tested compiled-to-WASM engine; re-implementing audio/video
codecs in Rust for this project would be enormous effort for no real gain over calling the
existing WASM build. That call is the media boundary decision — full rationale to be recorded
in `docs/ARCHITECTURE.md` once the community docs set is written; until then this paragraph is
the source of truth. This package is exposed to `apps/web` / `apps/desktop` through
`@conv.cat/engine`, the same way the Rust-backed converters are, so call sites don't need to
know which format is native-Rust and which is ffmpeg-wasm.

This package must never contain non-media conversion logic (units, images, text, CAD, …) —
those belong in `crates/conv-core`. It is currently a scaffold: no ffmpeg-wasm wiring yet.
