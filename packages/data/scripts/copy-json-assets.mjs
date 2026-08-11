#!/usr/bin/env node
// `tsc` only emits transformed `.ts` -> `.js`; it never copies non-TypeScript assets into
// `outDir`. This package's `src/units/*.json` catalog files are imported at runtime via
// `import ... with { type: 'json' }` (see src/units/index.ts), so without this step
// `dist/units/index.js` would reference relative JSON paths that don't exist in `dist/` at all —
// `pnpm build` would report success while shipping a broken package. Run after `tsc` as part of
// this package's `build` script.
import { cp, mkdir, readdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = fileURLToPath(new URL('..', import.meta.url));
const srcDir = join(packageRoot, 'src');
const distDir = join(packageRoot, 'dist');

async function copyJsonFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const srcPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      await copyJsonFiles(srcPath);
      continue;
    }
    if (!entry.name.endsWith('.json')) continue;

    const relPath = srcPath.slice(srcDir.length + 1);
    const destPath = join(distDir, relPath);
    await mkdir(dirname(destPath), { recursive: true });
    await cp(srcPath, destPath);
  }
}

await copyJsonFiles(srcDir);
