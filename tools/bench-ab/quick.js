// Porównanie przed/po z przebiegów `--quick`, wspólne dla `compare.js` i raportu.
//
// Każda z trzech macierzy trybu szybkiego to osobny przypadek, więc wynik jest per macierz:
// powtórzenia (kolejne kroki) ze wszystkich rund łączone osobno dla „przed” i „po”, stosunek
// median po/przed i 95% przedział z bootstrapu (losowanie ze zwracaniem w obrębie strony,
// stałe ziarno, więc wynik powtarzalny). Kroki jednej trajektorii nie są niezależne, więc
// przedział jest raczej za wąski; rozrzut między rundami (`perRound`) pokazuje dryf.
'use strict';

const fs = require('fs');
const path = require('path');
const D = require('../bench-viz/data.js');

const METRICS = [
  ['step_ns', 'krok'],
  ['compute_forces_ns', 'compute_forces'],
  ['compute_friction_ns', 'compute_friction'],
  ['integrate_ns', 'integrate'],
  ['wrap_around_ns', 'wrap_around'],
  ['rebuild_grid_ns', 'rebuild_grid'],
  ['update_shape_ns', 'update_shape'],
  // Pochodna: czas sił na kandydata w tym samym kroku (porównanie, gdy trajektorie się
  // rozjechały i stany mają inną liczbę kandydatów).
  ['forces_per_candidate', 'forces / kandydat'],
];

/** Kolumna przebiegu albo pochodna (`forces_per_candidate`, ns na kandydata na krok). */
function series(rec, col) {
  if (col === 'forces_per_candidate') {
    const f = rec.compute_forces_ns;
    const c = rec.candidates_per_step;
    return f?.length && c?.length ? f.map((x, i) => x / c[i]) : undefined;
  }
  return rec[col];
}

let parquet = null;
async function loadRun(file) {
  if (!parquet) {
    const [hyparquet, { compressors }] = await Promise.all([import('hyparquet'), import('hyparquet-compressors')]);
    parquet = { ...hyparquet, compressors };
  }
  const bytes = fs.readFileSync(file);
  return D.readRun(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), parquet, path.basename(file));
}

function bootstrap(before, after, iterations = 2000) {
  let seed = 12345;
  const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
  const resample = (xs) => xs.map(() => xs[Math.floor(rnd() * xs.length)]);
  const ratios = [];
  for (let i = 0; i < iterations; i++) ratios.push(D.median(resample(after)) / D.median(resample(before)));
  ratios.sort((a, b) => a - b);
  return [ratios[Math.floor(0.025 * iterations)], ratios[Math.floor(0.975 * iterations)]];
}

/**
 * `rounds`: `[{before, after}]` z nazwami plików w `dir`. Wynik: `{ seeds, metrics, bySeed }`,
 * `bySeed.get(seed)` = `{ n, offset, candidates: {before, after}, sameStates, metrics: { [kolumna]:
 * { before, after (mediany ns), ratio, lo, hi, perRound: [stosunki] } } }`.
 */
async function compareRounds(rounds, dir) {
  const sides = { before: [], after: [] };
  for (const r of rounds) {
    sides.before.push(await loadRun(path.join(dir, r.before)));
    sides.after.push(await loadRun(path.join(dir, r.after)));
  }
  const bySeed = new Map();
  const key = (rec) => `${rec.n}|${rec.spec_seed}|${rec.step_offset}`;
  const first = sides.before[0].records;
  for (const rec of first) {
    const recs = (side) => sides[side].map((run) => run.records.find((x) => key(x) === key(rec)));
    const b = recs('before');
    const a = recs('after');
    if (b.some((x) => !x) || a.some((x) => !x)) continue;
    const metrics = {};
    for (const [col] of METRICS) {
      const has = (x) => series(x, col)?.length;
      if (!has(b[0]) && !has(a[0])) continue;
      if (!has(b[0]) || !has(a[0])) {
        // Systemu nie ma po jednej stronie (np. nowy system): tylko mediana tam, gdzie jest.
        const side = has(b[0]) ? b : a;
        const m = D.median(side.flatMap((x) => series(x, col)));
        metrics[col] = { before: side === b ? m : NaN, after: side === a ? m : NaN, ratio: NaN, lo: NaN, hi: NaN, perRound: [] };
        continue;
      }
      const before = b.flatMap((x) => series(x, col));
      const after = a.flatMap((x) => series(x, col));
      const [lo, hi] = bootstrap(before, after);
      metrics[col] = {
        before: D.median(before),
        after: D.median(after),
        ratio: D.median(after) / D.median(before),
        lo,
        hi,
        perRound: b.map((x, i) => D.median(series(a[i], col)) / D.median(series(x, col))),
      };
    }
    const mean = (xs) => xs.reduce((s, x) => s + x, 0) / xs.length;
    const seq = (x) => x.candidates_per_step.join(',');
    bySeed.set(rec.spec_seed, {
      n: rec.n,
      offset: rec.step_offset,
      candidates: { before: mean(b.map((x) => x.candidates_mean)), after: mean(a.map((x) => x.candidates_mean)) },
      // Te same stany: identyczne sekwencje kandydatów w „przed” i „po”.
      sameStates: seq(b[0]) === seq(a[0]),
      metrics,
    });
  }
  return {
    rounds: rounds.length,
    revisions: { before: sides.before[0].revision, after: sides.after[0].revision },
    seeds: [...bySeed.keys()],
    bySeed,
  };
}

const num = (x, d = 3) => (Number.isFinite(x) ? x.toFixed(d) : '—');

function formatComparison(cmp) {
  const lines = [
    `${cmp.rounds} rund, przed ${cmp.revisions.before}, po ${cmp.revisions.after}; stosunek median po/przed [95% CI], rundy osobno`,
  ];
  for (const [seed, s] of cmp.bySeed) {
    lines.push(
      `macierz ${seed} (n=${s.n}, offset ${s.offset}): kandydaci ${num(s.candidates.before, 1)} → ${num(s.candidates.after, 1)}` +
        (s.sameStates ? ', te same stany' : ', INNE STANY (trajektoria się zmieniła)'),
    );
    for (const [col, label] of METRICS) {
      const m = s.metrics[col];
      if (!m) continue;
      const value = (x) => (col === 'forces_per_candidate' ? `${x.toFixed(2).padStart(7)} ns` : `${(x / 1e3).toFixed(1).padStart(7)} µs`);
      lines.push(
        `  ${label.padEnd(17)} ${value(m.before)} → ${value(m.after)}  ×${num(m.ratio)} [${num(m.lo)}, ${num(m.hi)}]  rundy ${m.perRound.map((x) => num(x)).join(' ')}`,
      );
    }
  }
  return lines;
}

module.exports = { METRICS, loadRun, compareRounds, formatComparison };
