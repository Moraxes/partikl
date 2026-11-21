---
id: 28
title: GPU compute port
summary: M1 portu symulacji na GPU wg plan_rozwoju_partikl_na_gpu.md (bez UI)
status: done
priority: soon
branch: compute-shader-1
created: 2026-11-21T00:51:19
worked: 2026-11-21T13:33:41
edited: 2026-11-21T13:33:41
links:
- kind: commit
  target: 4195c6a
  at: 2026-11-21T12:37:45
  note: 'fix(gpu): make the headless harness fail loudly and run any step count'
- kind: commit
  target: 70c66bb
  at: 2026-11-21T12:37:45
  note: 'build(gpu): embed the shaders in the binary, hot reload behind a dev feature'
- kind: commit
  target: bcf8d7b
  at: 2026-11-21T12:37:45
  note: 'fix(gpu): require --gpu for --gpu-deterministic, fix stale id comment'
- kind: commit
  target: cb39173
  at: 2026-11-21T12:59:26
  note: 'fix(gpu): draw each particle from its own vertices'
- kind: commit
  target: d340ac1
  at: 2026-11-21T13:31:13
  note: 'fix(gpu): wait for every thread to read the block sum before clearing it'
- kind: commit
  target: fb15142
  at: 2026-11-21T13:31:13
  note: 'fix(gpu): grow grid cells instead of panicking on large worlds'
- kind: commit
  target: 48fb8f9
  at: 2026-11-21T13:32:08
  note: 'fix(gpu): run the headless harness without pipelined rendering'
- kind: commit
  target: 66b9709
  at: 2026-11-21T13:33:33
  note: 'feat(bench): A/B comparison for the GPU bench'
---
Wynik: M1a i M1b gotowe (--gpu). Stan cząstek w buforze GPU rysowany bez kopii na CPU; siatka na GPU (histogram, suma prefiksowa Blellocha, scatter), siły w step_driver z regułami w rules/default_rules.wesl, warianty SimOptions przez shader defs, hot reload shaderów (cargo dev -- --gpu), tryb deterministyczny. Testy headless: krok vs wyrocznia f64 (wszystkie warianty, też duży świat), niezmienniki, powtarzalność bit w bit. Bench GPU do Parquet z A/B (AB_BENCH=gpu tools/bench-ab/ab.sh). Quick bench (µs/krok, M5): 9000: 96.8, 100k: 748, 500k: 4541. 100k+ w 60 FPS sprawdzone ręcznie w oknie.

## Poprawki po review
- harness: pomijanie testów tylko przy braku GPU; dowolna liczba kroków; bez pipelined rendering (pomiar czekał na GPU niepewnie),
- shadery wkompilowane w binarkę (src/gpu/shaders), file_watcher tylko w feature dev,
- render: numer cząstki i narożnik w wierzchołku (vertex_index nie zaczyna się od 0: linie przez okno),
- suma prefiksowa: bariera przed zerowaniem sumy bloku (wyścig gubił cząstki),
- duże okna: większe komórki zamiast asercji.

## Dalej
Czasy przebiegów (047), test rysowania (048), wyścig ładowania shaderów (049), refaktor (035).
