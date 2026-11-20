#!/usr/bin/env node
// Raport etapów optymalizacji z przebiegów benchmarków.
//
//   node tools/bench-report/report.js [manifest] [wyjście]
//
// Wymaga `npm install` w `tools/` (hyparquet). Domyślnie czyta `.data/perf-stages.json` i
// pisze `.worklog/reports/perf-report.md` (gałąź `worklog`). Manifest to lista etapów w kolejności; pierwszy jest punktem wyjścia. Pliki przebiegów są względne do
// `.data/bench-results/`, `grid` albo `sim` można pominąć, jeśli etap nie zmienia tej części:
//
//   { "stages": [
//     { "name": "punkt wyjścia", "commit": "8f682a0",
//       "sim": "sim-….parquet", "grid": "grid-….parquet", "note": "opcjonalnie" },
//     { "name": "…", "commit": "…", "sim": "…", "grid": "…",
//       "before": { "sim": "…", "grid": "…" } },
//     … ] }
//
// `before` to opcjonalny pomiar kodu poprzedniego etapu zrobiony tuż przed pomiarem etapu
// (bez dryfu warunków między dniami); bez niego etap porównuje się z przebiegami poprzedniego
// etapu. Porównanie jest parami: ten sam (n, macierz, offset) w obu przebiegach, stosunek
// median czasu nowy/stary, średnia geometryczna i 95% przedział ufności (rozkład normalny na
// logach). Wynik względem punktu wyjścia to iloczyn wyników kolejnych etapów.
'use strict';

const fs = require('fs');
const path = require('path');
const D = require('../bench-viz/data.js');

const root = path.resolve(__dirname, '../..');
const manifestPath = path.resolve(process.argv[2] ?? path.join(root, '.data/perf-stages.json'));
const outPath = path.resolve(process.argv[3] ?? path.join(root, '.worklog/reports/perf-report.md'));
const resultsDir = path.join(path.dirname(manifestPath), 'bench-results');

const METRICS = [
  ['sim: step', 'krok'],
  ['sim: compute_forces', 'compute_forces'],
  ['sim: compute_friction', 'compute_friction'],
  ['sim: integrate', 'integrate'],
  ['sim: wrap_around', 'wrap_around'],
  ['sim: update_shape', 'update_shape'],
  ['grid: neighbours', 'grid: neighbours'],
  ['grid: build', 'grid: build'],
  ['grid: move', 'grid: move'],
];

let parquet = null;
async function loadRun(file, bench) {
  if (!file) return null;
  if (!parquet) {
    const [hyparquet, { compressors }] = await Promise.all([import('hyparquet'), import('hyparquet-compressors')]);
    parquet = { ...hyparquet, compressors };
  }
  const bytes = fs.readFileSync(path.join(resultsDir, file));
  const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  const run = await D.readRun(buffer, parquet, file);
  if (run.bench !== bench) throw new Error(`${file}: oczekiwany przebieg ${bench}, jest ${run.bench}`);
  return run;
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
async function loadPair(files) {
  const sim = await loadRun(files.sim, 'sim');
  const grid = await loadRun(files.grid, 'grid');
  const data = D.buildDataset(sim, grid);
  return { simRun: sim, gridRun: grid, data, byKey: new Map(data.points.map((p) => [p.key, p])) };
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

async function main() {
const stages = [];
for (const s of manifest.stages) {
  stages.push({ ...s, ...(await loadPair(s)), before: s.before ? await loadPair(s.before) : null });
}
/** Z czym porównywać etap `i`: jego pomiar `before` albo przebiegi poprzedniego etapu. */
const reference = (i) => stages[i].before ?? stages[i - 1];

/** Stosunki nowy/stary dla metryki, pogrupowane: 'all' i `${n}|${offset}`. */
function compare(oldStage, newStage, metric) {
  const groups = new Map();
  const push = (k, x) => (groups.get(k) ?? groups.set(k, []).get(k)).push(x);
  for (const p of oldStage.data.points) {
    const q = newStage.byKey.get(p.key);
    const r = q && D.value(q, metric) / D.value(p, metric);
    if (!Number.isFinite(r) || r <= 0) continue;
    push('all', Math.log(r));
    push(`${p.n}|${p.offset}`, Math.log(r));
  }
  const out = new Map();
  for (const [k, logs] of groups) {
    const n = logs.length;
    const mean = logs.reduce((s, x) => s + x, 0) / n;
    const sd = n > 1 ? Math.sqrt(logs.reduce((s, x) => s + (x - mean) ** 2, 0) / (n - 1)) : 0;
    const h = (1.96 * sd) / Math.sqrt(n);
    out.set(k, { ratio: Math.exp(mean), lo: Math.exp(mean - h), hi: Math.exp(mean + h), count: n });
  }
  return out;
}

/** Ile wspólnych punktów sim ma identyczną sekwencję kandydatów (te same trajektorie). */
function sameTrajectories(a, b) {
  let same = 0;
  let total = 0;
  for (const p of a.data.points) {
    const q = b.byKey.get(p.key);
    const x = p.candidatesPerStep.sim;
    const y = q && q.candidatesPerStep.sim;
    if (!x || !y) continue;
    total++;
    if (x.length === y.length && x.every((v, i) => v === y[i])) same++;
  }
  return { same, total };
}

const num = (x, digits = 2) => (Number.isFinite(x) ? x.toFixed(digits).replace('.', ',') : '—');
const cell = (c) => (c ? num(c.ratio) : '—');
const ci = (c) => (c ? `${num(c.ratio)} [${num(c.lo)}; ${num(c.hi)}]` : '—');
const short = (rev) => (rev ? rev.replace(/-dirty$/, '*') : '—');
const table = (head, rows) =>
  [`| ${head.join(' | ')} |`, `|${head.map(() => '---').join('|')}|`, ...rows.map((r) => `| ${r.join(' | ')} |`)].join('\n');

const lines = [];
lines.push('# Raport wydajności', '');
lines.push(
  'Etapy optymalizacji symulacji i indeksu przestrzennego, mierzone benchmarkami `sim` i `grid`',
  '(`cargo bench`). Liczby to średnia geometryczna stosunku czasów nowy/stary na tych samych',
  '`(n, macierz, offset)`; poniżej 1 znaczy szybciej. Plik generuje',
  '`node tools/bench-report/report.js` z `.data/perf-stages.json`; nie edytować ręcznie.',
  '',
);

/** Iloczyn wyników etapów 1..i dla metryki; NaN, jeśli któryś etap jej nie ma. */
function cumulative(i, m) {
  let product = 1;
  for (let k = 1; k <= i; k++) {
    const c = compare(reference(k), stages[k], m).get('all');
    product *= c ? c.ratio : NaN;
  }
  return product;
}
const metricsPresent = METRICS.filter(([m]) => stages.some((s) => s.data.metrics.includes(m)));
const head = ['Etap', 'Commit', ...metricsPresent.map(([, label]) => label)];

lines.push('## Względem poprzedniego etapu', '');
lines.push(
  table(
    head,
    stages.map((s, i) => [
      s.name,
      `\`${s.commit}\``,
      ...metricsPresent.map(([m]) => (i === 0 ? '—' : cell(compare(reference(i), s, m).get('all')))),
    ]),
  ),
  '',
);

lines.push('## Względem punktu wyjścia', '');
lines.push('Iloczyn wyników kolejnych etapów.', '');
lines.push(
  table(
    head,
    stages.map((s, i) => [
      s.name,
      `\`${s.commit}\``,
      ...metricsPresent.map(([m]) => num(cumulative(i, m))),
    ]),
  ),
  '',
);

// Czasy bezwzględne kroku: mediana po macierzach w każdym (n, offset).
const ns = [...new Set(stages.flatMap((s) => s.data.ns))].sort((a, b) => a - b);
const offsets = [...new Set(stages.flatMap((s) => s.data.offsets))].sort((a, b) => a - b);
const simStages = stages.filter((s) => s.simRun);
if (simStages.length) {
  lines.push('## Czas kroku symulacji', '');
  lines.push(
    'Mediana po macierzach z median czasu kroku w µs, z przebiegów każdego etapu (różne dni, więc',
    'zawierają dryf warunków; zmiany etapów mierzą tabele wyżej).',
    '',
  );
  lines.push(
    table(
      ['n', 'offset', ...simStages.map((s) => s.name)],
      ns.flatMap((n) =>
        offsets.map((o) => [
          n,
          o,
          ...simStages.map((s) =>
            num(D.median(s.data.points.filter((p) => p.n === n && p.offset === o).map((p) => D.value(p, 'sim: step'))) / 1000, 0),
          ),
        ]),
      ),
    ),
    '',
  );
}

lines.push('## Etapy', '');
stages.forEach((s, i) => {
  lines.push(`### ${i}. ${s.name} (\`${s.commit}\`)`, '');
  if (s.note) lines.push(s.note, '');
  const runLine = (label, r) => `- ${label}${r.bench}: \`${r.source}\`, rewizja \`${short(r.revision)}\`, ${r.records.length} rekordów`;
  lines.push(
    [
      ...(s.before ? [s.before.simRun, s.before.gridRun].filter(Boolean).map((r) => runLine('przed, ', r)) : []),
      ...[s.simRun, s.gridRun].filter(Boolean).map((r) => runLine(s.before ? 'po, ' : '', r)),
    ].join('\n'),
    '',
  );
  if (i === 0) return;
  const prev = reference(i);
  if (s.before) lines.push('Porównanie z pomiarem „przed” zrobionym tuż przed pomiarem etapu.', '');
  const { same, total } = sameTrajectories(prev, s);
  if (total) {
    lines.push(
      same === total
        ? `Trajektorie identyczne jak przed zmianą (${same}/${total} punktów), porównanie na tych samych stanach.`
        : `Trajektorie różnią się od tych sprzed zmiany (${same}/${total} identycznych): porównanie sim jest na tych samych macierzach, ale innych stanach.`,
      '',
    );
  }
  lines.push(
    table(
      ['Metryka', 'ogółem [95% CI]', ...ns.map((n) => `n=${n}`)],
      metricsPresent
        .map(([m, label]) => [label, compare(prev, s, m)])
        .filter(([, c]) => c.has('all'))
        .map(([label, c]) => [
          label,
          ci(c.get('all')),
          ...ns.map((n) => offsets.map((o) => cell(c.get(`${n}|${o}`))).join(' / ')),
        ]),
    ),
    '',
    `Kolumny n: offsety ${offsets.join(' / ')}.`,
    '',
  );
});

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, lines.join('\n'));
console.log(`zapisano ${path.relative(process.cwd(), outPath)} (${stages.length} etapów)`);
}
