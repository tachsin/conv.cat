# @conv.cat/media

Video and audio conversion via ffmpeg-wasm. License: **MIT**.

This package stays TypeScript on purpose — it does not get ported to `crates/conv-core`, and
wrapping ffmpeg in Rust is explicitly out of scope for this project. See
[`docs/ARCHITECTURE.md` § The media boundary](../../docs/ARCHITECTURE.md#the-media-boundary-video-and-audio-stay-on-ffmpeg-wasm)
for the full rationale. This package is exposed to `apps/web` / `apps/desktop` through
`@conv.cat/engine`, the same way the Rust-backed converters are, so call sites don't need to
know which format is native-Rust and which is ffmpeg-wasm.

This package must never contain non-media conversion logic (units, images, text, …) —
those belong in `crates/conv-core`. It is currently a scaffold: no ffmpeg-wasm wiring yet.
