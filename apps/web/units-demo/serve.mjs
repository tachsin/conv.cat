#!/usr/bin/env node
// Zero-dependency static file server for the manual QA demo in this directory — see
// index.html's header comment. Not for production use. Serves the whole repo root over plain
// HTTP (not just apps/web/) because this demo's import map (index.html) points at
// packages/engine/dist and packages/data/dist directly: the browser's ES module loader (unlike
// Node's resolver) only understands http(s) and relative URLs, so those packages' built output
// has to be reachable from wherever this page is served, not just apps/web's own tree. Mirrors
// packages/engine/demo/serve.mjs's pattern; see that file for the fuller version of these
// comments.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../..', import.meta.url)); // repo root

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
};

const port = Number(process.env.PORT ?? 8788);

// Same fix as packages/engine/demo/serve.mjs, same reason: import maps don't apply inside a
// module Worker (verified empirically there — Chrome 151), so `@conv.cat/engine`'s
// `wasm/worker.js` (spawned by `WasmBackend`) can't resolve its own bare `import('conv-wasm')`
// via this page's import map. Rewritten in flight, on this one response only — the built file on
// disk is untouched.
const WORKER_PATH = '/packages/engine/dist/wasm/worker.js';
const WORKER_BARE_IMPORT = "import('conv-wasm')";
const WORKER_RESOLVED_IMPORT = "import('../../node_modules/conv-wasm/conv_wasm.js')";

const server = createServer((req, res) => {
  void handle(req, res);
});

async function handle(req, res) {
  try {
    const url = new URL(req.url ?? '/', 'http://localhost');

    // A real redirect, not an internal rewrite — see packages/engine/demo/serve.mjs's comment on
    // why this has to change the browser's actual location, not just serve different bytes at `/`.
    if (url.pathname === '/') {
      res.writeHead(302, { Location: '/apps/web/units-demo/index.html' }).end();
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
  console.log(`conv.cat units demo — http://localhost:${port}/`);
  console.log(
    'If the page fails to load: build first — pnpm --filter @conv.cat/data --filter @conv.cat/engine build ' +
      '(see this directory\'s README).',
  );
});
