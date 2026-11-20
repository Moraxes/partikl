---
id: 14
title: Siatka przez sortowanie
summary: 'Siatka sortowaniem przez zliczanie: krok ×0,97 / ×0,92 / ×0,89; złączenie systemów odrzucone'
status: done
priority: soon
branch: main
part_of: 12
created: 2026-11-20T20:29:45
worked: 2026-11-20T20:52:44
edited: 2026-11-20T20:52:44
links:
- kind: commit
  target: b25cb70
  at: 2026-11-20T20:50:45
  note: 'perf: rebuild the spatial grid every step by counting sort'
---
Wynik: siatka układana co krok od nowa sortowaniem przez zliczanie (`b25cb70`, system `rebuild_grid`). `wrap_around` + `rebuild_grid` 51 / 45 / 41 µs zamiast `wrap_around` 57 / 54 / 78 µs; krok ×0,97 / ×0,92 / ×0,89 (macierze 23 / 6 / 4). Złączenie tarcia, całkowania, zawijania i siatki w jeden system odrzucone (wolniejsze). Wersja równoległa niezrobiona: zakres pracy w puli Bevy kosztuje ok. 30 µs (zadanie 013), więcej niż cała przebudowa.

## Wyniki (`--quick`, 3 rundy na przemian, po/przed [95% CI])
| wariant | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| sortowanie przez zliczanie (`b25cb70`): krok | 0,97 [0,96; 0,99] | 0,92 [0,92; 0,93] | 0,89 [0,87; 0,91] |
| kandydaci przed → po | 35,9 → 35,9 | 136,3 → 129,3 | 1220 → 1101 |
| `compute_forces` na kandydata | 0,97 | 0,98 | 1,02 |
| jeden system zamiast czterech: krok | 1,04 [1,03; 1,06] | 1,07 [1,06; 1,08] | 1,00 [0,98; 1,03] |
| to samo: złączony system vs suma czterech | 95 vs ok. 71 µs | 87 vs ok. 62 µs | 88 vs ok. 61 µs |

Kolejność cząstek w komórce to teraz kolejność ECS (wcześniej kolejność wstawiania i przenoszenia), więc sumy sił dodają się w innej kolejności i trajektorie się rozjeżdżają: przy macierzach 6 i 4 część zysku kroku to mniej kandydatów w tych stanach, nie szybszy kod.

## Zmiany obok
- API siatki: `rebuild` zamiast `insert_entity`, `remove_entity`, `move_entity`; `get_neighbours` zwraca kopie `Neighbour`.
- Format wyników 1.1: kolumna `rebuild_grid_ns`; bench `grid`: operacja `rebuild` zamiast `move` (36,6 µs, `grid-2026-11-20-20-52-06`).
- Dane: etap w `.artifacts/perf-stages.json` (`quick`), raport `.artifacts/reports/perf-report.md`. Eksperyment ze złączeniem (bez commita): `sim-2026-11-20-20-51-09` → `-20-51-17`, `-20-51-30` → `-20-51-23`, `-20-51-37` → `-20-51-44`.

## Pierwotny plan
Cel: zastąpić `move_entity` dla każdej cząstki (`wrap_around` 50–80 µs), `Vec<Vec<Neighbour>>`, sloty i seryjne `ForceData::fill` jednym przebiegiem. Następny krok: indeks komórki dla każdej cząstki (równolegle), zliczanie i suma prefiksowa, rozłożenie wprost do `xs`, `ys`, `coeffs` w kolejności komórek; `compute_friction`, `integrate` i `wrap_around` złączone w jedno przejście po cząstkach (dziś cztery osobne przejścia po `Transform`, 40 B na cząstkę).

## Uwagi
- Publiczne API `SpatialGrid` używane przez bench `grid` i testy (`src/sim/grid/tests.rs`) trzeba przemyśleć: bench `grid` mierzy `move` i `build`, część operacji zniknie.
- Kolejność sumowania sił się zmieni, trajektorie się rozjadą (dopuszczalne).
