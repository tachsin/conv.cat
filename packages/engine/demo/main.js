// Manual QA harness only — see index.html's header comment. Plain JS, not built by `tsc`
// (this directory is outside `src/`, deliberately): it imports the package's own compiled
// `dist/` output, the same thing a real consumer would import, rather than re-typechecking
// source that's already covered by `pnpm typecheck`.
import { getEngine, ConvertError } from '../dist/index.js';

// getEngine() is async — see index.ts's doc comment on why (dynamic import so a bundler can
// split the unused backend out of the build). `engine` is resolved once in init() below and
// reused for every conversion after that, same as any other caller would.
let engine;

const engineInfoEl = document.getElementById('engine-info');
const formEl = document.getElementById('convert-form');
const fileInput = document.getElementById('file-input');
const fromSelect = document.getElementById('from-select');
const toSelect = document.getElementById('to-select');
const progressBar = document.getElementById('progress-bar');
const statusEl = document.getElementById('status');
const cancelButton = document.getElementById('cancel-button');
const downloadLink = document.getElementById('download-link');

let activeController = null;

async function init() {
  engine = await getEngine();
  const [formats, memoryCeilingBytes] = await Promise.all([
    engine.listFormats(),
    engine.getMemoryCeilingBytes(),
  ]);

  engineInfoEl.innerHTML = `
    <dt>Backend</dt><dd>${engine.backend}</dd>
    <dt>Memory ceiling</dt><dd>${
      memoryCeilingBytes === null ? 'none reported (native)' : `${memoryCeilingBytes.toLocaleString()} bytes`
    }</dd>
    <dt>Formats known</dt><dd>${formats.length}</dd>
  `;

  for (const format of formats) {
    const label = `${format.id} (${format.mime})`;
    if (format.canRead) fromSelect.add(new Option(label, format.id));
    if (format.canWrite) toSelect.add(new Option(label, format.id));
  }
}

formEl.addEventListener('submit', (event) => {
  event.preventDefault();
  void runConversion();
});

cancelButton.addEventListener('click', () => {
  activeController?.abort();
});

async function runConversion() {
  const file = fileInput.files?.[0];
  if (!file) return;

  statusEl.textContent = 'Converting…';
  progressBar.value = 0;
  downloadLink.style.display = 'none';
  cancelButton.disabled = false;
  activeController = new AbortController();

  try {
    const input = new Uint8Array(await file.arrayBuffer());
    // Read before convert(), not after: per the documented "input is consumed" contract (see
    // ConvEngine.convert's doc comment in types.ts), the WASM backend transfers input's
    // underlying buffer to the Worker rather than cloning it — input.byteLength reads 0 once
    // that transfer has happened, which is correct, expected behavior, not a bug. Confirmed
    // in-session: a first version of this line read it after convert() and printed "0 bytes in"
    // on every successful WASM-backend conversion.
    const inputByteLength = input.byteLength;
    const output = await engine.convert(input, fromSelect.value, toSelect.value, {
      onProgress: (fraction) => {
        progressBar.value = fraction;
      },
      signal: activeController.signal,
    });

    const url = URL.createObjectURL(new Blob([output]));
    downloadLink.href = url;
    downloadLink.download = `converted-${file.name}`;
    downloadLink.textContent = `Download result (${output.byteLength} bytes)`;
    downloadLink.style.display = '';
    statusEl.textContent = `Done — ${inputByteLength} bytes in, ${output.byteLength} bytes out, via the "${engine.backend}" backend.`;
  } catch (error) {
    if (error instanceof ConvertError) {
      statusEl.textContent = `Error [${error.kind}]: ${error.message}\n\n${JSON.stringify(error.details, null, 2)}`;
    } else {
      statusEl.textContent = `Unexpected error: ${String(error)}`;
    }
  } finally {
    cancelButton.disabled = true;
    activeController = null;
  }
}

init().catch((error) => {
  statusEl.textContent = `Failed to initialize engine: ${String(error)}`;
});
