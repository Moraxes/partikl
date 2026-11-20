---
id: 19
title: Limit sąsiadów w skupiskach
summary: 'Próbkowanie sąsiadów powyżej 512 kandydatów: krok ×0,47 przy gęstej macierzy; skupiska bardziej zaszumione (do oceny)'
status: doing
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:37:16
edited: 2026-11-20T21:38:09
links:
- kind: commit
  target: f2ea0ba
  at: 2026-11-20T21:37:16
  note: 'perf: sample neighbours of particles with more than 512 candidates'
---
Wynik: próbkowanie sąsiadów powyżej 512 kandydatów (`f2ea0ba`): krok ×1,01 / ×0,97 / ×0,47 (macierze 23 / 6 / 4); 23 i 6 nie są próbkowane. Gęste skupiska bardziej zaszumione. Do decyzji użytkownika: limit (`NEIGHBOUR_CAP` w `src/sim/forces.rs`) albo wycofanie.

## Wyniki (`--quick`, 3 rundy na przemian)
| wariant | macierz 23 | macierz 6 | macierz 4 | kandydaci m4 po 4096 krokach |
|---|---|---|---|---|
| limit 256, krok | 1,01 | 0,91 (próbkowana część komórek) | 0,36 [0,36; 0,37] | 1209 → 730 |
| limit 512 (`f2ea0ba`), krok | 1,01 [1,00; 1,02] | 0,97 [0,95; 0,98] | 0,47 [0,46; 0,47] | 1209 → 1041 |

## Wygląd
Obrazy pozycji po 4096 krokach (góra przed, dół po), `.artifacts/reports/img/`:
- `019-m4-przed-vs-limit256.png`: zwarte, pełne skupiska prawie znikają, zostaje kilka luźnych, postrzępionych; mniej skupisk.
- `019-m4-przed-vs-limit512.png`: zwarte skupiska nadal są, część większych bardziej postrzępiona.
- `019-m6-przed-vs-limit512.png`: bez zmian charakteru (macierz 6 nie jest próbkowana).
Jedna klatka chaotycznej trajektorii, więc ocena przybliżona; w aplikacji warto obejrzeć ruch (szum próbkowania może być widoczny jako drganie skupisk).

## Szczegóły
- Co n-ty wektor (n = kandydaci / 512 w górę) z wagą n, przesunięcie wyboru co krok: średnia po n krokach to pełna suma (test na gęstym skupisku, 840 kroków).
- Pary liczone raz biorą wagę cząstki, która liczy parę; odpychanie też próbkowane (nie da się go oddzielić bez liczenia odległości).
- Komórki poniżej limitu idą osobnym wariantem jądra (bez wag), bo wariant z wagą per cząstka kosztował 5–9% przy macierzach 23 i 6.

## Pierwotny plan
Cel: ograniczony koszt na cząstkę w gęstych skupiskach przy mniejszej zmianie wyglądu niż ograniczenie gęstości. Następny krok: w `compute_forces`, gdy liczba kandydatów cząstki przekracza K (np. 256), przegląd co k-tego wektora z wagą k; zmierzyć czas przy macierzy 4 i obejrzeć skupiska.

## Uwagi
- Stały wybór podzbioru daje stały błąd (to samo pomijane co krok); przesunięcie wyboru co krok zamienia go w szum.
- Odpychanie bliskich cząstek nie powinno być próbkowane, inaczej skupiska mogą się zapadać.
