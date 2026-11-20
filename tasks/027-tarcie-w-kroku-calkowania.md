---
id: 27
title: Tarcie w kroku całkowania
summary: Tarcie z prędkości po kroku, dokładny zanik kwadratowy; koszt +5 µs, stabilności przy 100k nie naprawia
status: doing
priority: now
branch: main
created: 2026-11-20T23:24:58
worked: 2026-11-20T23:34:08
edited: 2026-11-20T23:27:16
links:
- kind: commit
  target: f855ea1
  at: 2026-11-20T23:34:08
  note: 'fix: apply quadratic friction to the post-step velocity with its exact decay'
---
Wynik (niezacommitowane, do przejrzenia): zmiana tylko w `compute_friction` + `friction_acceleration`, 3 testy w `src/sim/tests.rs`. Koszt: `compute_friction` 7 → 12 µs, krok ×1,016 / ×0,990 / ×1,032 (trajektorie inne). Stabilność przy n=100000: niższe maksima prędkości, odsetek szybkich cząstek bez wyraźnej zmiany: tarcie nie było przyczyną chaosu.

## Pomiar `--quick` (3 rundy, przed f2ea0ba)
| macierz | krok | compute_friction | forces / kandydat |
|---|---|---|---|
| 23 | ×1,016 [1,001; 1,027] | 7,5 → 12,4 µs | ×0,986 |
| 6 | ×0,990 [0,980; 1,000] | 7,1 → 11,9 µs | ×1,069 |
| 4 | ×1,032 [1,013; 1,043] | 7,1 → 11,9 µs | ×0,883 |

`wrap_around` ×0,8 we wszystkich (bez zmian w kodzie: prawdopodobnie układ binarki). Rundy: sim-2026-11-20-23-25-47/53, 23-26-04/25-59, 23-26-10/26-15.

## Sonda n=100000, `spec-2026-11-20-21-48-03.ron`, 1500 kroków
| krok | przed: >1000 j/s / max | po: >1000 j/s / max |
|---|---|---|
| 250 | 1,4% / 2893 | 3,6% / 3155 |
| 500 | 10,6% / 6340 | 10,0% / 3269 |
| 750 | 16,2% / 7690 | 24,5% / 4092 |
| 1000 | 23,2% / 8927 | 34,2% / 4855 |
| 1250 | 34,9% / 7735 | 39,9% / 5076 |
| 1500 | 44,1% / 9742 | 38,4% / 5747 |

