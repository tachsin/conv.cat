// Manual QA harness only — see index.html's header comment. Plain JS, not built by `tsc` (this
// directory is outside `src/`, deliberately, same convention as packages/engine/demo/main.js):
// it imports the packages' own compiled `dist/` output, the same thing a real consumer would
// import, rather than re-typechecking source that's already covered by `pnpm typecheck`.
import { getEngine, convertUnit, ConvertError } from '@conv.cat/engine';
import { UNIT_CATEGORIES, getUnitCategory } from '@conv.cat/data';

let engine;

const engineInfoEl = document.getElementById('engine-info');
const formEl = document.getElementById('convert-form');
const categorySelect = document.getElementById('category-select');
const fromSelect = document.getElementById('from-select');
const toSelect = document.getElementById('to-select');
const valueInput = document.getElementById('value-input');
const resultEl = document.getElementById('result');
const statusEl = document.getElementById('status');

function populateUnitSelects(categoryId) {
  const category = getUnitCategory(categoryId);
  fromSelect.innerHTML = '';
  toSelect.innerHTML = '';
  for (const unit of category.units) {
    const label = unit.symbol ? `${unit.name} (${unit.symbol})` : unit.name;
    fromSelect.add(new Option(label, unit.id));
    toSelect.add(new Option(label, unit.id));
  }
  // Default to.value to a different unit than from, when there's more than one, so the first
  // conversion someone tries isn't a same-unit no-op.
  if (toSelect.options.length > 1) {
    toSelect.selectedIndex = 1;
  }
}

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

  for (const categoryId of UNIT_CATEGORIES) {
    const category = getUnitCategory(categoryId);
    categorySelect.add(new Option(category.name, categoryId));
  }
  populateUnitSelects(categorySelect.value);
}

categorySelect.addEventListener('change', () => {
  populateUnitSelects(categorySelect.value);
});

formEl.addEventListener('submit', (event) => {
  event.preventDefault();
  void runConversion();
});

async function runConversion() {
  statusEl.textContent = 'Converting…';
  resultEl.textContent = '—';

  try {
    const result = await convertUnit(
      engine,
      categorySelect.value,
      fromSelect.value,
      toSelect.value,
      valueInput.value,
    );
    resultEl.textContent = String(result);
    statusEl.textContent = `Done — via the "${engine.backend}" backend.`;
  } catch (error) {
    if (error instanceof ConvertError) {
      statusEl.textContent = `Error [${error.kind}]: ${error.message}\n\n${JSON.stringify(error.details, null, 2)}`;
    } else {
      statusEl.textContent = `Unexpected error: ${String(error)}`;
    }
  }
}

init().catch((error) => {
  statusEl.textContent = `Failed to initialize engine: ${String(error)}`;
});
