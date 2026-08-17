#!/usr/bin/env node
// Wraps `next dev` with an auto-picked free port. Conductor runs several worktrees of this same
// repo side by side, each with the identical `apps/web/package.json` — a hardcoded port means the
// second workspace's `pnpm dev` just fails with EADDRINUSE against the first one's server (Next's
// dev CLI does not auto-increment on a taken port, unlike some other dev servers). Scanning for a
// free port here means `pnpm dev` always works, in any workspace, without manual `--port` juggling.

import { createServer } from 'node:net';
import { spawn } from 'node:child_process';

const BASE_PORT = Number(process.env.PORT) || 3011;
// Configurable so a machine running an unusual number of workspaces side by side can widen the
// search instead of the script just giving up.
const MAX_ATTEMPTS = Number(process.env.PORT_SCAN_MAX_ATTEMPTS) || 50;

function isPortFree(port) {
  return new Promise((resolve) => {
    const server = createServer();
    server.once('error', () => resolve(false));
    server.once('listening', () => server.close(() => resolve(true)));
    server.listen(port, '0.0.0.0');
  });
}

async function findFreePort(startPort) {
  for (let offset = 0; offset < MAX_ATTEMPTS; offset += 1) {
    const port = startPort + offset;
    if (await isPortFree(port)) return port;
  }
  const endPort = startPort + MAX_ATTEMPTS - 1;
  console.error(
    `\n  conv.cat dev: no free port in range ${startPort}-${endPort}. ` +
      `Set PORT to start from elsewhere, or PORT_SCAN_MAX_ATTEMPTS to widen the search.\n`,
  );
  process.exit(1);
}

const port = await findFreePort(BASE_PORT);
if (port !== BASE_PORT) {
  console.log(`\n  conv.cat dev: port ${BASE_PORT} is busy (likely another workspace) — using ${port} instead.\n`);
}

const child = spawn('next', ['dev', '--port', String(port)], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

child.on('exit', (code) => process.exit(code ?? 0));
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => child.kill(signal));
}
