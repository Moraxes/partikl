---
id: 51
title: Odchudzenie bench common
summary: benches/common tylko dla benchu gpu; własne argumenty gpu
status: done
priority: soon
branch: compute-shader-1
part_of: 50
created: 2026-11-21T15:00:57
worked: 2026-11-21T15:02:54
edited: 2026-11-21T15:02:54
links:
- kind: commit
  target: e5783d6
  at: 2026-11-21T15:02:54
  note: 'refactor(bench): keep only what the gpu bench uses in benches/common'
---
Cel: usunąć z benches/common resztki benchów CPU (--n, --offsets, --matrices, Sample, Progress, format_table, quantile, QUICK_N, opis QUICK_SEEDS pod CPU) i dać benchowi gpu spójne argumenty (--sizes, --steps, --batch, --quick, warianty). Bez zmiany formatu wyników (A/B i bench-viz działają dalej). Wynik: benches/common to scena (WORLD, rng, particle_spec), median/nanos i zapis Parquet (Output bez BenchArgs); argumenty w benches/gpu/main.rs (--seed, --types, --warmup, --measured, --out-dir, --quick). --quick bez zmian znaczenia (macierz 23, 100 pomiarów), A/B ze starszą rewizją zgodne.
