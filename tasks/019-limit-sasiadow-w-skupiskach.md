---
id: 19
title: Limit sąsiadów w skupiskach
summary: Przy ponad K kandydatach co k-ty sąsiad z wagą k; koszt na cząstkę ograniczony, szum sił w skupiskach
status: todo
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
---
Cel: ograniczony koszt na cząstkę w gęstych skupiskach przy mniejszej zmianie wyglądu niż ograniczenie gęstości. Następny krok: w `compute_forces`, gdy liczba kandydatów cząstki przekracza K (np. 256), przegląd co k-tego wektora z wagą k; zmierzyć czas przy macierzy 4 i obejrzeć skupiska.

## Uwagi
- Stały wybór podzbioru daje stały błąd (to samo pomijane co krok); przesunięcie wyboru co krok zamienia go w szum.
- Odpychanie bliskich cząstek nie powinno być próbkowane, inaczej skupiska mogą się zapadać.
