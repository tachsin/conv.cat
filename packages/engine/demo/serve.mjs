#!/usr/bin/env node
// Zero-dependency static file server for the manual QA demo in this directory. Not for
// production use — no caching headers, no compression, no directory-listing hardening beyond the
// one traversal check below. Its only job is serving this package's own `dist/` and
// `node_modules/` over plain HTTP with correct MIME types, because the browser's ES module
// loader (unlike Node's resolver) only understands http(s) and relative URLs — see index.html's
// import map comment for why that matters here specifically.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

// fileURLToPath on a directory URL (this one ends in "/") keeps the trailing separator, so
// `root` itself already ends with it — the traversal check below relies on that.
const root = fileURLToPath(new URL('..', import.meta.url)); // packages/engine/

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  // The MIME type WebAssembly.instantiateStreaming actually checks for — see
  // crates/conv-wasm/pkg/conv_wasm.js's __wbg_load, and check-wasm-size.sh's neighbor doc for
  // the same requirement in production. Getting this wrong doesn't break the demo (there's a
  // slower instantiate() fallback with a console warning) but would silently mask exactly the
  // kind of server-config mistake this demo exists to catch early.
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
};

const port = Number(process.env.PORT ?? 8787);

// wasm/worker.ts's `import('conv-wasm')` is correct for a real, bundler-based consumer (Vite,
// webpack, Next.js all resolve a Worker's bare specifiers themselves) — see that file's own
// comments. It is NOT correct for this bundler-free demo: verified empirically (Chrome 151, this
// session) that the page's `<script type="importmap">` does not apply inside a module Worker it
// creates, so the worker's bare `'conv-wasm'` import fails to resolve at runtime with "Failed to
// resolve module specifier" — import maps are scoped to the Window that declares them, and
// nothing here promotes that scope into the Worker's own module graph. Rather than carry that
// workaround into packages/engine's real source (which would leak a demo-only concern into
// production code every real consumer has to look past), this server rewrites just this one
// response in flight. `dist/` on disk is untouched; only the bytes sent over HTTP for this
// specific file differ from what's on disk.
const WORKER_PATH = '/dist/wasm/worker.js';
const WORKER_BARE_IMPORT = "import('conv-wasm')";
const WORKER_RESOLVED_IMPORT = "import('../../node_modules/conv-wasm/conv_wasm.js')";

const server = createServer((req, res) => {
  void handle(req, res);
});

async function handle(req, res) {
  try {
    const url = new URL(req.url ?? '/', 'http://localhost');

    // A real redirect, not an internal rewrite: index.html's own relative URLs (`./main.js`,
    // `../node_modules/...`) are resolved by the browser against whatever URL it thinks it's
    // at. Serving demo/index.html's *content* at `/` while telling the browser it's still at
    // `/` breaks every relative reference on the page (`./main.js` would resolve to `/main.js`,
    // not `/demo/main.js`) — the redirect is what makes the browser's location actually become
    // `/demo/index.html`, so the page's own relative paths resolve the way its author wrote them.
    if (url.pathname === '/') {
      res.writeHead(302, { Location: '/demo/index.html' }).end();
      return;
    }

    const requestPath = decodeURIComponent(url.pathname);
    const filePath = normalize(join(root, requestPath));

    if (!filePath.startsWith(root)) {
      res.writeHead(403).end('Forbidden');
      return;
    }

    const info = await stat(filePath);
    if (info.isDirectory()) {
      res.writeHead(404).end('Not found');
      return;
    }

    let body = await readFile(filePath);
    if (url.pathname === WORKER_PATH) {
      const patched = body.toString('utf8').replace(WORKER_BARE_IMPORT, WORKER_RESOLVED_IMPORT);
      body = Buffer.from(patched, 'utf8');
    }

    const contentType = MIME_TYPES[extname(filePath)] ?? 'application/octet-stream';
    res.writeHead(200, { 'Content-Type': contentType, 'Cache-Control': 'no-store' });
    res.end(body);
  } catch {
    res.writeHead(404).end('Not found');
  }
}

server.listen(port, () => {
  console.log(`conv-wasm demo harness — http://localhost:${port}/`);
  console.log('If the page fails to load the engine: build first (see this directory\'s README).');
});
