// Dane wizualizatora: parsowanie JSONL z benchmarków, mediany, cechy macierzy i PCA.
// Bez zależności i bez DOM, żeby dało się to sprawdzić w node:
//   node -e 'const d = require("./tools/bench-viz/data.js"); ...'
'use strict';

(function (root) {
  const SIM_SYSTEMS = ['compute_forces', 'compute_friction', 'integrate', 'wrap_around', 'update_shape'];

  function median(values) {
    const v = values.filter(Number.isFinite).sort((a, b) => a - b);
    if (v.length === 0) return NaN;
    const m = v.length >> 1;
    return v.length % 2 ? v[m] : (v[m - 1] + v[m]) / 2;
  }

  /** Rekordy z tekstu JSONL; błąd wskazuje numer linii. */
  function parseJsonl(text, name = 'plik') {
    const records = [];
    text.split('\n').forEach((line, i) => {
      if (line.trim() === '') return;
      try {
        records.push(JSON.parse(line));
      } catch (err) {
        throw new Error(`${name}, linia ${i + 1}: ${err.message}`);
      }
    });
    return records;
  }

  /** Przebiegi: rekordy pogrupowane po `run.bench` i `run.started`. */
  function groupRuns(records, source = '') {
    const runs = new Map();
    for (const r of records) {
      if (!r.run || !r.run.bench) continue;
      const key = `${r.run.bench}|${r.run.started}`;
      if (!runs.has(key)) {
        runs.set(key, {
          key,
          bench: r.run.bench,
          started: r.run.started,
          revision: r.run.revision,
          source,
          records: [],
        });
      }
      runs.get(key).records.push(r);
    }
    return [...runs.values()];
  }

  const pointKey = (n, seed, offset) => `${n}|${seed}|${offset}`;

  /**
   * Punkty `(n, macierz, offset)` z wybranych przebiegów sim i grid (dowolny może być null).
   * Metryki to mediany czasów w ns z mierzonych kroków/powtórzeń (po rozgrzewce).
   */
  function buildDataset(simRun, gridRun) {
    const points = new Map();
    const matrices = new Map();
    const metrics = new Set();

    function point(r) {
      const key = pointKey(r.n, r.spec_seed, r.step_offset);
      if (!points.has(key)) {
        points.set(key, {
          key,
          n: r.n,
          seed: r.spec_seed,
          offset: r.step_offset,
          values: {},
          candidates: {},
          candidatesPerStep: {},
        });
      }
      if (!matrices.has(r.spec_seed)) {
        matrices.set(r.spec_seed, { seed: r.spec_seed, placementSeed: r.placement_seed, matrix: r.interactions });
      }
      return points.get(key);
    }

    for (const r of simRun ? simRun.records : []) {
      const p = point(r);
      p.values['sim: step'] = median(r.step_ns);
      metrics.add('sim: step');
      for (const s of SIM_SYSTEMS) {
        if (!r[`${s}_ns`]) continue;
        p.values[`sim: ${s}`] = median(r[`${s}_ns`]);
        metrics.add(`sim: ${s}`);
      }
      p.candidates.sim = r.candidates_mean;
      p.candidatesPerStep.sim = r.candidates_per_step;
    }
    for (const r of gridRun ? gridRun.records : []) {
      const p = point(r);
      p.values[`grid: ${r.op}`] = median(r.ns);
      metrics.add(`grid: ${r.op}`);
      p.candidates.grid = r.candidates_mean;
    }

    const all = [...points.values()];
    const uniq = (f) => [...new Set(all.map(f))].sort((a, b) => a - b);
    const features = matrixFeatures([...matrices.values()]);
    return {
      points: all,
      matrices,
      features,
      metrics: [...metrics],
      ns: uniq((p) => p.n),
      offsets: uniq((p) => p.offset),
      seeds: uniq((p) => p.seed),
    };
  }

  /** Ława metryki: `kandydaci` są z tego samego benchu co metryka czasu. */
  function benchOf(metric) {
    return metric.startsWith('grid:') ? 'grid' : 'sim';
  }

  function value(point, metric) {
    if (metric === 'kandydaci (sim)') return point.candidates.sim;
    if (metric === 'kandydaci (grid)') return point.candidates.grid;
    return point.values[metric];
  }

  function candidatesFor(point, metric) {
    return point.candidates[benchOf(metric)];
  }

  /** Symetryczna macierz: wartości i wektory własne (kolumny), malejąco. Metoda Jacobiego. */
  function eigenSymmetric(matrix) {
    const n = matrix.length;
    const a = matrix.map((row) => row.slice());
    const v = Array.from({ length: n }, (_, i) => Array.from({ length: n }, (_, j) => (i === j ? 1 : 0)));
    for (let sweep = 0; sweep < 100; sweep++) {
      let off = 0;
      for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) off += a[i][j] * a[i][j];
      if (off < 1e-20) break;
      for (let p = 0; p < n; p++) {
        for (let q = p + 1; q < n; q++) {
          if (Math.abs(a[p][q]) < 1e-300) continue;
          const theta = (a[q][q] - a[p][p]) / (2 * a[p][q]);
          const t = Math.sign(theta || 1) / (Math.abs(theta) + Math.sqrt(theta * theta + 1));
          const c = 1 / Math.sqrt(t * t + 1);
          const s = t * c;
          for (let k = 0; k < n; k++) {
            const akp = a[k][p];
            const akq = a[k][q];
            a[k][p] = c * akp - s * akq;
            a[k][q] = s * akp + c * akq;
          }
          for (let k = 0; k < n; k++) {
            const apk = a[p][k];
            const aqk = a[q][k];
            a[p][k] = c * apk - s * aqk;
            a[q][k] = s * apk + c * aqk;
          }
          for (let k = 0; k < n; k++) {
            const vkp = v[k][p];
            const vkq = v[k][q];
            v[k][p] = c * vkp - s * vkq;
            v[k][q] = s * vkp + c * vkq;
          }
        }
      }
    }
    const order = [...Array(n).keys()].sort((i, j) => a[j][j] - a[i][i]);
    return {
      values: order.map((i) => a[i][i]),
      vectors: order.map((i) => v.map((row) => row[i])),
    };
  }

  /** PCA wektorów (wiersze): składowe, udział wariancji i rzuty na `k` pierwszych. */
  function pca(rows, k = 3) {
    const d = rows[0].length;
    const mean = Array.from({ length: d }, (_, j) => rows.reduce((s, r) => s + r[j], 0) / rows.length);
    const centered = rows.map((r) => r.map((x, j) => x - mean[j]));
    const cov = Array.from({ length: d }, (_, i) =>
      Array.from({ length: d }, (_, j) => centered.reduce((s, r) => s + r[i] * r[j], 0) / Math.max(1, rows.length - 1)),
    );
    const { values, vectors } = eigenSymmetric(cov);
    const total = values.reduce((s, x) => s + Math.max(0, x), 0) || 1;
    return {
      mean,
      covariance: cov,
      values,
      components: vectors.slice(0, k),
      explained: values.slice(0, k).map((x) => Math.max(0, x) / total),
      projections: centered.map((r) => vectors.slice(0, k).map((vec) => vec.reduce((s, x, j) => s + x * r[j], 0))),
    };
  }

  /**
   * Cechy każdej macierzy: współczynniki, ślad (przyciąganie własnego typu), średnia, średnia
   * poza przekątną, asymetria ‖A − Aᵀ‖, min, max oraz PC1–PC3. Zwraca nazwy cech i mapę
   * seed → {cecha: wartość}.
   */
  function matrixFeatures(entries) {
    const byseed = new Map();
    if (entries.length === 0) return { names: [], bySeed: byseed, pca: null };
    const t = entries[0].matrix.length;
    const names = [];
    for (let i = 0; i < t; i++) for (let j = 0; j < t; j++) names.push(`a[${i}][${j}]`);
    names.push('ślad', 'średnia', 'średnia poza przekątną', 'asymetria', 'min', 'max');

    for (const { seed, matrix } of entries) {
      const flat = matrix.flat();
      const f = {};
      for (let i = 0; i < t; i++) for (let j = 0; j < t; j++) f[`a[${i}][${j}]`] = matrix[i][j];
      let trace = 0;
      let off = 0;
      let asym = 0;
      for (let i = 0; i < t; i++) {
        trace += matrix[i][i];
        for (let j = 0; j < t; j++) {
          if (i !== j) off += matrix[i][j];
          asym += (matrix[i][j] - matrix[j][i]) ** 2;
        }
      }
      f['ślad'] = trace;
      f['średnia'] = flat.reduce((s, x) => s + x, 0) / flat.length;
      f['średnia poza przekątną'] = t > 1 ? off / (t * t - t) : 0;
      f['asymetria'] = Math.sqrt(asym);
      f['min'] = Math.min(...flat);
      f['max'] = Math.max(...flat);
      byseed.set(seed, f);
    }

    let result = null;
    if (entries.length >= 2) {
      result = pca(entries.map((e) => e.matrix.flat()), Math.min(3, t * t));
      entries.forEach((e, i) => {
        result.projections[i].forEach((x, k) => {
          byseed.get(e.seed)[`PC${k + 1}`] = x;
        });
      });
      result.explained.forEach((_, k) => names.unshift(`PC${result.explained.length - k}`));
    }
    return { names, bySeed: byseed, pca: result };
  }

  const api = {
    SIM_SYSTEMS,
    median,
    parseJsonl,
    groupRuns,
    buildDataset,
    benchOf,
    value,
    candidatesFor,
    eigenSymmetric,
    pca,
    matrixFeatures,
  };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.BenchData = api;
})(typeof window !== 'undefined' ? window : globalThis);
