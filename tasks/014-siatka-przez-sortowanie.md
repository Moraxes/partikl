---
id: 14
title: Siatka przez sortowanie
summary: Siatka budowana co krok sortowaniem przez zliczanie, jedno równoległe przejście z tarciem, całkowaniem i zawijaniem
status: todo
priority: soon
branch: main
part_of: 12
created: 2026-11-20T20:29:45
worked: 2026-11-20T20:50:45
links:
- kind: commit
  target: b25cb70
  at: 2026-11-20T20:50:45
  note: 'perf: rebuild the spatial grid every step by counting sort'
---
Cel: zastąpić `move_entity` dla każdej cząstki (`wrap_around` 50–80 µs), `Vec<Vec<Neighbour>>`, sloty i seryjne `ForceData::fill` jednym przebiegiem. Następny krok: indeks komórki dla każdej cząstki (równolegle), zliczanie i suma prefiksowa, rozłożenie wprost do `xs`, `ys`, `coeffs` w kolejności komórek; `compute_friction`, `integrate` i `wrap_around` złączone w jedno przejście po cząstkach (dziś cztery osobne przejścia po `Transform`, 40 B na cząstkę).

## Uwagi
- Publiczne API `SpatialGrid` używane przez bench `grid` i testy (`src/sim/grid/tests.rs`) trzeba przemyśleć: bench `grid` mierzy `move` i `build`, część operacji zniknie.
- Kolejność sumowania sił się zmieni, trajektorie się rozjadą (dopuszczalne).
