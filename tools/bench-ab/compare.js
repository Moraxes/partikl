#!/usr/bin/env node
// Porównanie przed/po z przebiegów `--quick` (`ab.sh`): per macierz stosunek median po/przed
// z powtórzeń wszystkich rund, z 95% przedziałem bootstrapowym.
//
//   node tools/bench-ab/compare.js '[{"before":"sim-….parquet","after":"sim-….parquet"}, …]'
//
// Pliki względne do `.artifacts/bench/`. Logika współdzielona z raportem (`quick.js`).
'use strict';

const path = require('path');
const Q = require('./quick.js');

const root = path.resolve(__dirname, '../..');

(async () => {
  const rounds = JSON.parse(process.argv[2]);
  const cmp = await Q.compareRounds(rounds, path.join(root, '.artifacts/bench'));
  for (const line of Q.formatComparison(cmp)) console.log(line);
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
