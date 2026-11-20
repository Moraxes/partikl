---
id: 17
title: Wspólne obliczenia par
summary: 'Pary raz powyżej 600 tys. par na krok: compute_forces na kandydata ×0,84 / ×0,75 (macierze 6 / 4)'
status: done
priority: later
branch: main
part_of: 12
blocked_by:
- 15
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:23:46
edited: 2026-11-20T21:23:45
links:
- kind: commit
  target: e38c3d2
  at: 2026-11-20T21:23:04
  note: 'perf: compute each close pair once for dense states'
- kind: report
  target: .artifacts/reports/perf-report.md
  at: 2026-11-20T21:23:46
  note: sekcja Tryb szybki
---
Wynik: pary liczone raz powyżej 600 tys. par na krok (`e38c3d2`): `compute_forces` na kandydata ×0,84 (macierz 6) i ×0,75 (macierz 4); przy małej gęstości bez zmian (tryb jednostronny). Zawsze symetrycznie: ×1,26 przy macierzy 23, dlatego próg.

## Wyniki (`--quick`, rundy na przemian)
| wariant | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| zawsze pary raz, `compute_forces` (3 rundy) | 1,26 [1,25; 1,28] | 1,04 [1,03; 1,05] | 0,80 [0,78; 0,81] |
| próg 600 tys. (`e38c3d2`), `compute_forces` na kandydata (4 rundy) | 1,00 [1,00; 1,01] | 0,84 [0,84; 0,86] | 0,75 [0,75; 0,76] |
| to samo, krok | 1,00 | 1,06 | 0,97 |
| kandydaci przed → po | 25,2 → 25,2 | 98,7 → 127,4 | 864,5 → 1118 |

Zmiana kolejności sumowania rozjeżdża trajektorie; nowe stany macierzy 6 i 4 mają więcej kandydatów, więc czas kroku miesza kod ze stanem. Stąd porównanie na kandydata (`forces / kandydat` w `tools/bench-ab`); stały narzut `compute_forces` (ok. 40–50 µs) rozłożony na więcej kandydatów też obniża tę liczbę (szacunkowo ok. 6% przy macierzy 6, ok. 1,5% przy 4).

## Jak działa
- Siatka: `half_runs(c)`: przebiegi komórek o większym indeksie bez szwu i przebiegi przez szew; test pokrycia każdej pary komórek dokładnie raz.
- Siła na drugą cząstkę do bufora wątku (`Mutex<PairBuffer>`, o jeden więcej niż wątków, zerowany przy pierwszym użyciu w kroku), bufory sumowane po zakresie zadań.
- Próg z pracy poprzedniego kroku (pierwszy krok liczy pary raz); test obu trybów (dwa kroki z rzędu).

Dane: etap w `.artifacts/perf-stages.json` (`quick`), raport `.artifacts/reports/perf-report.md`; przebieg „zawsze pary raz” w `.artifacts/bench/` (2026-11-20 21:20).

## Pierwotny plan
Cel: przyspieszyć jądro przy gęstych macierzach (4: 78% kroku w `compute_forces`). Siła nie jest symetryczna (`coeff[a][b] ≠ coeff[b][a]`), ale odległość, pierwiastek, dzielenia i kształt jądra są wspólne dla pary i,j. Następny krok: przegląd połowy sąsiedztwa (komórka z sobą i połowa sąsiednich) z dodawaniem do obu cząstek.

## Uwagi
- Zapis do obu cząstek koliduje z równoległością: bufory sił na wątek i suma na końcu albo kolorowanie komórek (komórki tego samego koloru nie mają wspólnych sąsiadów).
- Zależy od kolejności komórek i tablic z sortowania.
