---
id: 16
title: Mniejsze komórki siatki
summary: 'Komórki R/2, sąsiedztwo 5×5: krok ×1,07 / ×0,91 / ×0,85; tańszy podział sił ×0,93 przy małej gęstości'
status: done
priority: later
branch: main
part_of: 12
blocked_by:
- 14
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:17:16
edited: 2026-11-20T21:17:16
links:
- kind: commit
  target: 7c69d0c
  at: 2026-11-20T21:14:19
  note: 'perf: split forces into equal particle ranges sized by last step''s work'
- kind: commit
  target: 5ac5374
  at: 2026-11-20T21:15:37
  note: 'perf: grid cells of half the interaction radius with a 5×5 neighbourhood'
- kind: report
  target: .artifacts/reports/perf-report.md
  at: 2026-11-20T21:17:16
  note: sekcja Tryb szybki
---
Wynik: komórki R/2 i sąsiedztwo 5×5 (`5ac5374`): krok ×1,07 / ×0,91 / ×0,85 (macierze 23 / 6 / 4; +15 µs, −31 µs, −162 µs). Po drodze tańszy podział pracy sił (`7c69d0c`): krok ×0,93 przy macierzy 23, bez zmian przy 6 i 4. Zasięg 3 (komórki R/3, 7×7) odrzucony.

## Wyniki (`--quick`, rundy na przemian, krok po/przed [95% CI])
| wariant | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| podział po równo według pracy z poprzedniego kroku, zasięg 1 (`7c69d0c`), te same stany | 0,93 [0,91; 0,95] | 1,02 [1,01; 1,03] | 1,02 [0,99; 1,04] |
| zasięg 2 (`5ac5374`), 4 rundy | 1,07 [1,06; 1,09] | 0,91 [0,90; 0,92] | 0,85 [0,83; 0,86] |
| kandydaci z siatki przy zasięgu 2 | 35,9 → 25,2 | 129 → 99 | 1101 → 865 |
| zasięg 3 względem 2 | 1,18 [1,17; 1,19] | 1,08 [1,07; 1,09] | 1,14 [1,13; 1,15] |
| zasięg 2 z planem według kandydatów komórek (sumy w oknie), odrzucone | 1,19 | 1,02 | 0,86 |

## Ustalenia
- Narożników sąsiedztwa 5×5 nie da się pominąć: komórka (±2, ±2) leży ok. 0,71R od komórki pytającej (pierwotny plan zakładał inaczej).
- Zawijanie przy sąsiedztwie szerszym niż komórka wymaga parzystej liczby komórek w każdej osi (inaczej szew nie jest okresowy); bok komórki w osi dobrany tak, dla 2560×1440 bez zmian.
- Plan pracy przeglądający wszystkie komórki kosztował 16 µs (2304 komórki) i 40–50 µs (9216 komórek), także w wersji z sumami w oknie; zastąpiony podziałem po równo z liczbą zakresów z pracy poprzedniego kroku (zliczanej przez zadania).

Dane: etapy w `.artifacts/perf-stages.json` (`quick`), raport `.artifacts/reports/perf-report.md`; przebiegi odrzuconych wariantów w `.artifacts/bench/` z 2026-11-20 21:06–21:17.

## Pierwotny plan
Cel: mniej kandydatów poza zasięgiem. Dziś 3×3 komórki o boku R to obszar 9R² przy zasięgu πR² ≈ 3,1R² (ok. 35% trafień). Komórki R/2 z 5×5 to 6,25R², bez czterech narożnych ok. 5,25R². Następny krok: po przebudowie siatki parametr rozmiaru komórki i porównanie `--quick`; przebiegi w wierszu dalej ciągłe (5 komórek wiersza to jeden przebieg).

## Uwagi
- `INTERACTION_RADIUS` jest dziś jednocześnie rozmiarem komórki (`config.rs`); trzeba to rozdzielić.
- Więcej przebiegów na zapytanie (5 zamiast 3) i krótsze przebiegi: przy małej gęstości narzut może zjeść zysk.
