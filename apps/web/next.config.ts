import type { NextConfig } from 'next';

// Cross-origin-isolation headers, off in dev.
//
// Nothing this app ships *today* needs COOP/COEP — conv-wasm is single-threaded and
// cancellation is same-thread cooperative (see packages/engine/README.md "Cross-origin
// isolation (COEP/COOP)"). They're set anyway, in production only, because the first WASM
// thread or SharedArrayBuffer-backed feature (packages/media's ffmpeg-wasm, or genuine
// cross-thread cancellation) will need them, and retrofitting cross-origin isolation onto a
// shipped site after the fact is the kind of change that breaks third-party embeds without
// warning. Setting it now, while there is nothing yet to conflict with, is cheaper than
// fighting that fire later. See docs/ARCHITECTURE.md "The media boundary" for the same note.
const crossOriginIsolationHeaders = [
  { key: 'Cross-Origin-Opener-Policy', value: 'same-origin' },
  { key: 'Cross-Origin-Embedder-Policy', value: 'require-corp' },
];

const nextConfig: NextConfig = {
  reactStrictMode: true,

  headers() {
    if (process.env.NODE_ENV !== 'production') {
      return [];
    }
    return [{ source: '/:path*', headers: crossOriginIsolationHeaders }];
  },
};

export default nextConfig;
