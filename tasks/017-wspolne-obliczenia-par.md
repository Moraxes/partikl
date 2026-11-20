---
id: 17
title: Wspólne obliczenia par
summary: Odległość i jądro liczone raz na parę, oba współczynniki; ok. połowa pracy jądra w gęstych skupiskach
status: todo
priority: later
branch: main
part_of: 12
blocked_by:
- 15
created: 2026-11-20T20:29:45
---
Cel: przyspieszyć jądro przy gęstych macierzach (4: 78% kroku w `compute_forces`). Siła nie jest symetryczna (`coeff[a][b] ≠ coeff[b][a]`), ale odległość, pierwiastek, dzielenia i kształt jądra są wspólne dla pary i,j. Następny krok: przegląd połowy sąsiedztwa (komórka z sobą i połowa sąsiednich) z dodawaniem do obu cząstek.

## Uwagi
- Zapis do obu cząstek koliduje z równoległością: bufory sił na wątek i suma na końcu albo kolorowanie komórek (komórki tego samego koloru nie mają wspólnych sąsiadów).
- Zależy od kolejności komórek i tablic z sortowania.
