---
id: 16
title: Mniejsze komórki siatki
summary: 'Komórki R/2 i sąsiedztwo 5×5 bez narożników: o 30–40% mniej sprawdzanych par'
status: doing
priority: later
branch: main
part_of: 12
blocked_by:
- 14
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:04:42
---
Cel: mniej kandydatów poza zasięgiem. Dziś 3×3 komórki o boku R to obszar 9R² przy zasięgu πR² ≈ 3,1R² (ok. 35% trafień). Komórki R/2 z 5×5 to 6,25R², bez czterech narożnych ok. 5,25R². Następny krok: po przebudowie siatki parametr rozmiaru komórki i porównanie `--quick`; przebiegi w wierszu dalej ciągłe (5 komórek wiersza to jeden przebieg).

## Uwagi
- `INTERACTION_RADIUS` jest dziś jednocześnie rozmiarem komórki (`config.rs`); trzeba to rozdzielić.
- Więcej przebiegów na zapytanie (5 zamiast 3) i krótsze przebiegi: przy małej gęstości narzut może zjeść zysk.
