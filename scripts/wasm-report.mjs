#!/usr/bin/env node
// wasm-report — what is inside the WebAssembly artifact, and which code put it there.
//
// `.github/scripts/check-wasm-size.sh` tells you the payload grew. This tells you WHY: which
// exports exist, and how many bytes each Rust module contributes. Run it when the size budget
// fails, before raising the budget — the usual answer to "why did this grow 40 KB" is a
// dependency nobody meant to ship, and that is invisible from the total alone.
//
// Zero dependencies, on purpose: the same reason build-all.sh and the demo server are
// dependency-free. Reading a size report should not require every contributor to first install
// a Rust analysis toolchain.
//
// ─── The two-artifact trick, and why the numbers differ ──────────────────────────────────────
//
// Attribution needs symbol names. The SHIPPED artifact has none: wasm-opt strips the `name`
// section in release, which is a large part of why it is only ~130 KB. Verified in-session —
// wasm-pack `--release` AND `--profiling` both emit a byte-identical, name-free binary.
//
// So this script reads two builds and is explicit about which number came from where:
//
//   crates/conv-wasm/pkg/conv_wasm_bg.wasm            SHIPPED. Authoritative totals, real
//                                                     export list, section breakdown.
//   target/wasm32-unknown-unknown/release/*.wasm      SYMBOLS. LLVM-optimised but pre-wasm-opt
//                                                     and pre-wasm-bindgen, so it keeps `name`.
//                                                     Used ONLY for relative attribution.
//
// Treat attribution as proportional, not absolute: the symbol build's code section is roughly
// 2.5x the shipped one because wasm-opt has not run. A module that is 40% of the code there is
// approximately 40% of the code that ships — it is not literally that many bytes.
//
// Usage:
//   node scripts/wasm-report.mjs            report from existing artifacts
//   node scripts/wasm-report.mjs --build    build both artifacts first (what `pnpm wasm:report` does)
//   node scripts/wasm-report.mjs --json     machine-readable output
//   node scripts/wasm-report.mjs --top 30   show more individual functions (default 15)

import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const SHIPPED = join(repoRoot, 'crates/conv-wasm/pkg/conv_wasm_bg.wasm');
const SYMBOLS = join(repoRoot, 'target/wasm32-unknown-unknown/release/conv_wasm.wasm');
const BUDGET_FILE = join(repoRoot, '.wasm-size-budget');

const argv = process.argv.slice(2);
const opts = {
  build: argv.includes('--build'),
  json: argv.includes('--json'),
  top: Number(argv[argv.indexOf('--top') + 1]) || 15,
};

// ─── wasm binary parsing ─────────────────────────────────────────────────────────────────────
// Only the parts this report needs. The format is stable and simple enough that a parser is
// less of a liability than a dependency: a section is (id: u8, size: u32, payload).

const SECTION_NAMES = {
  1: 'type', 2: 'import', 3: 'function', 4: 'table', 5: 'memory', 6: 'global',
  7: 'export', 8: 'start', 9: 'elem', 10: 'code', 11: 'data', 12: 'datacount', 13: 'tag',
};

class Reader {
  constructor(buf, pos = 0) { this.buf = buf; this.pos = pos; }
  u8() { return this.buf[this.pos++]; }
  /** LEB128 unsigned varint — the length/index encoding used throughout the format. */
  leb() {
    let result = 0, shift = 0, byte;
    do {
      byte = this.buf[this.pos++];
      result |= (byte & 0x7f) << shift;
      shift += 7;
    } while (byte & 0x80);
    return result >>> 0;
  }
  bytes(n) { const b = this.buf.subarray(this.pos, this.pos + n); this.pos += n; return b; }
  str() { return Buffer.from(this.bytes(this.leb())).toString('utf8'); }
}

function parseSections(buf) {
  const r = new Reader(buf, 8); // skip magic + version
  const sections = [];
  while (r.pos < buf.length) {
    const id = r.u8();
    const size = r.leb();
    const start = r.pos;
    let name = SECTION_NAMES[id] ?? `unknown(${id})`;
    if (id === 0) {
      const sub = new Reader(buf, start);
      name = `custom("${sub.str()}")`;
    }
    sections.push({ id, name, size, start, end: start + size });
    r.pos = start + size;
  }
  return sections;
}

/** Exported functions/memories/etc — the artifact's actual public surface. */
function parseExports(buf, sections) {
  const sec = sections.find((s) => s.id === 7);
  if (!sec) return [];
  const r = new Reader(buf, sec.start);
  const count = r.leb();
  const KINDS = { 0: 'func', 1: 'table', 2: 'memory', 3: 'global' };
  const out = [];
  for (let i = 0; i < count; i++) {
    const name = r.str();
    const kind = KINDS[r.u8()] ?? 'other';
    out.push({ name, kind, index: r.leb() });
  }
  return out;
}

/** Number of imported functions — function indices are offset by this in the code section. */
function countImportedFunctions(buf, sections) {
  const sec = sections.find((s) => s.id === 2);
  if (!sec) return 0;
  const r = new Reader(buf, sec.start);
  const count = r.leb();
  let funcs = 0;
  for (let i = 0; i < count; i++) {
    r.str(); r.str();                       // module, field
    const kind = r.u8();
    if (kind === 0) { r.leb(); funcs++; }   // typeidx
    else if (kind === 1) { r.leb(); r.leb(); if (r.buf[r.pos - 1] === 1) r.leb(); } // table
    else if (kind === 2) { const fl = r.leb(); r.leb(); if (fl & 1) r.leb(); }       // memory
    else if (kind === 3) { r.leb(); r.u8(); }                                        // global
  }
  return funcs;
}

/** Per-function body sizes, keyed by function index. */
function parseCodeSizes(buf, sections, importedFuncs) {
  const sec = sections.find((s) => s.id === 10);
  if (!sec) return new Map();
  const r = new Reader(buf, sec.start);
  const count = r.leb();
  const sizes = new Map();
  for (let i = 0; i < count; i++) {
    const bodySize = r.leb();
    sizes.set(importedFuncs + i, bodySize);
    r.pos += bodySize;
  }
  return sizes;
}

/** Function index → symbol name, from the `name` custom section's subsection 1. */
function parseFunctionNames(buf, sections) {
  const sec = sections.find((s) => s.name === 'custom("name")');
  const names = new Map();
  if (!sec) return names;
  const r = new Reader(buf, sec.start);
  r.str(); // "name"
  while (r.pos < sec.end) {
    const subId = r.u8();
    const subSize = r.leb();
    const subEnd = r.pos + subSize;
    if (subId === 1) {
      const count = r.leb();
      for (let i = 0; i < count; i++) names.set(r.leb(), r.str());
      return names;
    }
    r.pos = subEnd;
  }
  return names;
}

// ─── Rust symbol demangling (legacy `_ZN..E` scheme, which is what rustc emits for wasm) ─────

const ESCAPES = {
  $LT$: '<', $GT$: '>', $LP$: '(', $RP$: ')', $C$: ',', $BP$: '*', $RF$: '&',
  $SP$: '@', $u20$: ' ', $u27$: "'", $u5b$: '[', $u5d$: ']', $u7b$: '{', $u7d$: '}',
  $u3b$: ';', $u5c$: '\\', $u7e$: '~',
};

function unescapeIdent(s) {
  let out = s.replace(/\$[A-Za-z0-9_]{1,4}\$/g, (m) => ESCAPES[m] ?? m);
  return out.replace(/\.\./g, '::').replace(/^\.$/, '.');
}

/**
 * Partial demangler for the v0 scheme (`_R…`), which rustc uses for a lot of std/alloc/core.
 *
 * Full v0 is a whole grammar (generics, backrefs, lifetimes) and decoding it properly is not
 * worth it here — this report only needs the *path*, to answer "whose code is this". So this
 * walks to the crate root marker `C`, skips its base-62 disambiguator, and then reads the
 * length-prefixed identifiers that follow.
 *
 * Without this, every v0 symbol falls through as an opaque `_RNv…` blob and gets bucketed as an
 * unknown third-party dependency — which is exactly backwards, since almost all of them are the
 * Rust standard library. Skipping it made "other deps" look like 79% of the binary.
 */
function demangleV0(sym) {
  const cIdx = sym.indexOf('C');
  if (cIdx === -1) return null;
  let i = cIdx + 1;
  if (sym[i] === 's') {                       // optional disambiguator: s<base62>_
    const end = sym.indexOf('_', i);
    if (end === -1) return null;
    i = end + 1;
  }
  const parts = [];
  while (i < sym.length) {
    let len = 0;
    const digitStart = i;
    while (i < sym.length && sym[i] >= '0' && sym[i] <= '9') { len = len * 10 + Number(sym[i]); i++; }
    if (i === digitStart || len === 0) break;
    parts.push(sym.slice(i, i + len));
    i += len;
    // Nested paths re-introduce namespace/backref markers; stop at the first one we can't read
    // as a plain identifier rather than guessing.
    if (i < sym.length && !(sym[i] >= '0' && sym[i] <= '9')) break;
  }
  return parts.length ? parts.join('::') : null;
}

function demangle(sym) {
  const cleaned = sym.replace(/\.llvm\.\d+$/, '');
  if (cleaned.startsWith('_R')) return demangleV0(cleaned) ?? cleaned;
  if (!cleaned.startsWith('_ZN')) return cleaned;
  let i = 3;
  const parts = [];
  while (i < cleaned.length) {
    let len = 0;
    while (i < cleaned.length && cleaned[i] >= '0' && cleaned[i] <= '9') {
      len = len * 10 + Number(cleaned[i]); i++;
    }
    if (len === 0) break;
    parts.push(cleaned.slice(i, i + len));
    i += len;
  }
  // Trailing `17h<16 hex>` is the disambiguating hash, never useful in a report.
  if (parts.length && /^h[0-9a-f]{16}$/.test(parts[parts.length - 1])) parts.pop();
  return parts.map(unescapeIdent).join('::');
}

/** Removes `<…>` generic parameters, which otherwise shred the path into fragments like `T,A>`. */
function stripGenerics(s) {
  let out = '', depth = 0;
  for (const ch of s) {
    if (ch === '<') { depth++; continue; }
    if (ch === '>') { if (depth > 0) depth--; continue; }
    if (depth === 0) out += ch;
  }
  return out;
}

/**
 * The module a symbol should be *attributed* to.
 *
 * Three shapes have to be handled separately, and getting any of them wrong fragments the report
 * into hundreds of meaningless buckets:
 *
 *   `<Concrete as Trait>::method`      legacy trait impl — the owner is Concrete, not the trait
 *   `impl Trait for Concrete`          the v0 spelling of the same thing — owner is after ` for `
 *   `a::b::c::function`                plain path — drop the trailing function name
 *
 * Depth is capped so this groups by module instead of exploding per function.
 */
function attributionKey(demangled, depth = 4) {
  let path = demangled;
  if (path.startsWith('<')) {
    const inner = path.slice(1);
    const asIdx = inner.indexOf(' as ');
    path = asIdx !== -1 ? inner.slice(0, asIdx) : inner.split('>')[0];
  } else if (path.startsWith('impl ')) {
    const forIdx = path.indexOf(' for ');
    path = forIdx !== -1 ? path.slice(forIdx + 5) : path.slice(5);
  }
  path = stripGenerics(path).replace(/^&+/, '').trim();
  let segments = path.split('::').map((s) => s.trim()).filter(Boolean);
  // A bare `_` segment is a generated item (closure, shim, nested impl) that v0 encodes without
  // a real name. It is not a module, and keeping it produces useless buckets like `js_sys::_::_`,
  // so truncate the path there and attribute the bytes to the nearest real module.
  const blank = segments.indexOf('_');
  if (blank !== -1) segments = segments.slice(0, blank);
  else if (segments.length > 1) segments.pop(); // drop the function/method name
  return segments.slice(0, depth).join('::') || '(generated items)';
}

const RUNTIME_CRATES = new Set([
  'core', 'alloc', 'std', 'compiler_builtins', 'hashbrown', 'rustc_std_workspace_core',
  'dlmalloc', 'panic_abort', 'unwind', 'libc', 'memchr', 'adler', 'miniz_oxide',
]);

/**
 * Buckets a module path into the four groups someone actually reasons about when deciding what
 * to cut: our code, the bindings layer, the Rust runtime we can't remove, and real dependencies.
 */
function ownerOf(key) {
  if (key.startsWith('__wbindgen') || key.startsWith('__wbg') || key === '(wasm-bindgen shims)') {
    return 'bindgen glue';
  }
  if (/\bwasm_bindgen\b|\bjs_sys\b|\bweb_sys\b/.test(key)) return 'bindgen glue';
  if (key.startsWith('___rust') || key.startsWith('__rust') || key.startsWith('__extern')) {
    return 'Rust runtime';
  }
  const crate = key.split('::')[0];
  if (crate === 'conv_core' || crate === 'conv_wasm') return 'ours';
  if (RUNTIME_CRATES.has(crate)) return 'Rust runtime';
  return 'other deps';
}

// ─── formatting ──────────────────────────────────────────────────────────────────────────────

const kb = (n) => `${(n / 1024).toFixed(1)} KB`;
const pct = (n, total) => (total ? `${((n / total) * 100).toFixed(1)}%` : '0.0%');
const isTTY = process.stdout.isTTY;
const c = {
  bold: isTTY ? '\x1b[1m' : '', dim: isTTY ? '\x1b[2m' : '',
  green: isTTY ? '\x1b[32m' : '', yellow: isTTY ? '\x1b[33m' : '', reset: isTTY ? '\x1b[0m' : '',
};

function bar(fraction, width = 24) {
  const filled = Math.max(0, Math.min(width, Math.round(fraction * width)));
  return '█'.repeat(filled) + '·'.repeat(width - filled);
}

function readBudget() {
  if (!existsSync(BUDGET_FILE)) return null;
  const lines = readFileSync(BUDGET_FILE, 'utf8')
    .split('\n').map((l) => l.trim()).filter((l) => l && !l.startsWith('#'));
  const n = Number(lines[lines.length - 1]);
  return Number.isFinite(n) ? n : null;
}

// ─── build ───────────────────────────────────────────────────────────────────────────────────

function build() {
  const wasmPackVersionFile = join(repoRoot, '.wasm-pack-version');
  process.stderr.write(`${c.dim}Building shipped artifact (wasm-pack --release)…${c.reset}\n`);
  execFileSync('wasm-pack', ['build', 'crates/conv-wasm', '--target', 'web', '--out-dir', 'pkg', '--release'],
    { cwd: repoRoot, stdio: 'inherit' });
  process.stderr.write(`${c.dim}Building symbol artifact (cargo --release, keeps the name section)…${c.reset}\n`);
  execFileSync('cargo', ['build', '-p', 'conv-wasm', '--target', 'wasm32-unknown-unknown', '--release'],
    { cwd: repoRoot, stdio: 'inherit' });
  void wasmPackVersionFile;
}

// ─── main ────────────────────────────────────────────────────────────────────────────────────

if (opts.build) build();

const missing = [
  [SHIPPED, 'wasm-pack build crates/conv-wasm --target web --out-dir pkg --release'],
  [SYMBOLS, 'cargo build -p conv-wasm --target wasm32-unknown-unknown --release'],
].filter(([p]) => !existsSync(p));

if (missing.length) {
  process.stderr.write(`\n✗ wasm-report: missing build artifact(s).\n\n`);
  for (const [p, cmd] of missing) {
    process.stderr.write(`    ${p.replace(repoRoot + '/', '')}\n      build with: ${cmd}\n`);
  }
  process.stderr.write(`\n  Or let this script do it:  node scripts/wasm-report.mjs --build\n\n`);
  process.exit(2);
}

const shippedBuf = readFileSync(SHIPPED);
const shippedSections = parseSections(shippedBuf);
const exports_ = parseExports(shippedBuf, shippedSections);
const budget = readBudget();

const symBuf = readFileSync(SYMBOLS);
const symSections = parseSections(symBuf);
const symNames = parseFunctionNames(symBuf, symSections);
const symImported = countImportedFunctions(symBuf, symSections);
const symCode = parseCodeSizes(symBuf, symSections, symImported);

// Aggregate per-function sizes into module buckets.
const byModule = new Map();
const functions = [];
let attributedTotal = 0;
let describeOnlyBytes = 0;
for (const [idx, size] of symCode) {
  const raw = symNames.get(idx);

  // `__wbindgen_describe*` functions exist only so the wasm-bindgen CLI can read the type
  // signatures out of the module at build time. It deletes them afterwards, so they are NOT in
  // the shipped artifact — but they are ~45% of this build's code section. Counting them would
  // make the single largest "cost" in the report a thing that never ships, and would make every
  // other percentage wrong. Excluded, and reported separately below.
  if (raw && /__wbindgen_describe/.test(raw)) { describeOnlyBytes += size; continue; }

  const demangled = raw ? demangle(raw) : `(anonymous func ${idx})`;
  // An unmangled symbol is not Rust source: it is a wasm-bindgen generated wrapper (the
  // `convert` / `convertoptions_*` exports). Grouping each one as its own "module" is what
  // produced hundreds of one-function buckets.
  const unmangled = raw && !raw.startsWith('_ZN') && !raw.startsWith('_R');
  const key = !raw ? '(unnamed)' : unmangled ? '(wasm-bindgen shims)' : attributionKey(demangled);
  byModule.set(key, (byModule.get(key) ?? 0) + size);
  functions.push({ name: demangled, size });
  attributedTotal += size;
}

const modules = [...byModule.entries()]
  .map(([key, size]) => ({ key, size, owner: ownerOf(key) }))
  .sort((a, b) => b.size - a.size);

const byOwner = new Map();
for (const m of modules) byOwner.set(m.owner, (byOwner.get(m.owner) ?? 0) + m.size);

functions.sort((a, b) => b.size - a.size);

const shippedTotal = shippedBuf.length;
const dataSection = shippedSections.find((s) => s.id === 11);
const codeSection = shippedSections.find((s) => s.id === 10);

if (opts.json) {
  console.log(JSON.stringify({
    shipped: {
      bytes: shippedTotal,
      budget,
      sections: shippedSections.map((s) => ({ name: s.name, bytes: s.size })),
      exports: exports_,
    },
    attribution: {
      note: 'Proportional, from the pre-wasm-opt symbol build — not literal shipped bytes.',
      codeBytesAnalysed: attributedTotal,
      excludedDescribeScaffoldingBytes: describeOnlyBytes,
      byOwner: Object.fromEntries(byOwner),
      modules,
      topFunctions: functions.slice(0, opts.top),
    },
  }, null, 2));
  process.exit(0);
}

// ── 1. shipped artifact ──
console.log(`\n${c.bold}Shipped artifact${c.reset}  ${c.dim}crates/conv-wasm/pkg/conv_wasm_bg.wasm${c.reset}`);
console.log(`  ${c.bold}${shippedTotal} bytes (${kb(shippedTotal)})${c.reset}` +
  (budget ? `  of ${budget} budget — ${kb(budget - shippedTotal)} spare` : ''));

console.log(`\n${c.bold}Sections${c.reset} ${c.dim}(what the bytes physically are)${c.reset}`);
for (const s of [...shippedSections].sort((a, b) => b.size - a.size)) {
  if (s.size < 64) continue;
  console.log(`  ${s.name.padEnd(26)} ${String(s.size).padStart(7)}  ${pct(s.size, shippedTotal).padStart(6)}  ${c.dim}${bar(s.size / shippedTotal)}${c.reset}`);
}
if (dataSection && codeSection) {
  console.log(`\n  ${c.dim}code = compiled logic; data = static bytes (unit tables, strings, catalogs).${c.reset}`);
  console.log(`  ${c.dim}A big data section means the catalog is the payload, and no code change will shrink it.${c.reset}`);
}

// ── 2. exports ──
const exportedFns = exports_.filter((e) => e.kind === 'func');
console.log(`\n${c.bold}Exports${c.reset}  ${c.dim}${exports_.length} total, ${exportedFns.length} functions — the artifact's public surface${c.reset}`);
for (const e of exports_.filter((x) => x.kind !== 'func')) {
  console.log(`  ${c.dim}[${e.kind}]${c.reset} ${e.name}`);
}
const appFns = exportedFns.filter((e) => !/^__(wbindgen|wbg)/.test(e.name));
const glueFns = exportedFns.filter((e) => /^__(wbindgen|wbg)/.test(e.name));
for (const e of appFns) console.log(`  ${c.green}${e.name}${c.reset}`);
if (glueFns.length) {
  console.log(`  ${c.dim}+ ${glueFns.length} wasm-bindgen runtime exports (__wbindgen_*) — allocator//glue, not API${c.reset}`);
}

// ── 3. attribution ──
console.log(`\n${c.bold}Code size by module${c.reset}  ${c.dim}${kb(attributedTotal)} of compiled code analysed${c.reset}`);
console.log(`${c.yellow}  Proportional, not literal:${c.reset} ${c.dim}measured on the pre-wasm-opt build (the shipped one has no symbols).${c.reset}`);
console.log(`  ${c.dim}Read the percentages, not the byte counts.${c.reset}`);
console.log(`  ${c.dim}Excluded ${kb(describeOnlyBytes)} of __wbindgen_describe* scaffolding — build-time only, never ships.${c.reset}\n`);

for (const [owner, size] of [...byOwner.entries()].sort((a, b) => b[1] - a[1])) {
  console.log(`  ${owner.padEnd(14)} ${String(size).padStart(7)}  ${pct(size, attributedTotal).padStart(6)}  ${c.dim}${bar(size / attributedTotal)}${c.reset}`);
}

console.log(`\n${c.bold}  Our code, by module${c.reset}`);
const ours = modules.filter((m) => m.owner === 'ours');
if (ours.length === 0) console.log(`    ${c.dim}(none attributed)${c.reset}`);
for (const m of ours) {
  console.log(`    ${m.key.padEnd(46)} ${String(m.size).padStart(7)}  ${pct(m.size, attributedTotal).padStart(6)}`);
}

console.log(`\n${c.bold}  Everything else, top 10${c.reset}`);
for (const m of modules.filter((x) => x.owner !== 'ours').slice(0, 10)) {
  console.log(`    ${c.dim}${m.key.padEnd(46)}${c.reset} ${String(m.size).padStart(7)}  ${pct(m.size, attributedTotal).padStart(6)}`);
}

// ── 4. biggest individual functions ──
console.log(`\n${c.bold}Largest functions${c.reset} ${c.dim}(top ${opts.top})${c.reset}`);
for (const f of functions.slice(0, opts.top)) {
  const name = f.name.length > 88 ? `${f.name.slice(0, 85)}…` : f.name;
  console.log(`  ${String(f.size).padStart(6)}  ${name}`);
}

console.log(`\n${c.dim}Budget check: ./.github/scripts/check-wasm-size.sh${c.reset}`);
console.log(`${c.dim}Machine-readable: node scripts/wasm-report.mjs --json${c.reset}\n`);
