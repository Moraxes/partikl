---
id: 25
title: Rozsuwanie nakładających się cząstek
summary: Poprawka pozycji par bliżej niż zasięg odpychania zamiast sprężyny
status: done
priority: soon
branch: main
blocked_by:
- 22
created: 2026-11-20T23:12:22
worked: 2026-11-20T23:44:23
edited: 2026-11-20T23:44:23
links:
- kind: commit
  target: 00f9005
  at: 2026-11-20T23:44:23
  note: 'feat: overlap resolution by position correction as an overlap mode'
- kind: commit
  target: 00f9005
  at: 2026-11-20T23:44:23
---
Wynik: `--overlap resolve` / klawisz O. Samo rozsuwanie chaosu nie usuwa (bez sprężyny prędkości dalej rosną: sztywne jest też przyciąganie sumowane po tysiącach sąsiadów, nachylenie coeff/10 na sąsiada). Z uśrednianiem (024) stabilne i skupiska wyraźnie luźniejsze niż sprężyna z uśrednianiem.

## Zmiana względem planu
Poprawki liczone w przejściu sił (wariant `compute_forces::<_, RESOLVE>`: zamiast sprężyny suma poprawek), stosowane przy zapisie do ECS przed całkowaniem. Bez drugiego przejścia po sąsiadach i bez nieaktualnej siatki; przebiegi 3×3 i osobny system niepotrzebne. Poprawka uśredniona po nakładających się parach cząstki (Jacobi z uśrednianiem), bez wagi próbkowania.

## Sonda n=100000, `spec-2026-11-20-21-48-03.ron`, krok 1500
| wariant | prędkość med / p99 / max [j/s] | >1000 j/s | najpełniejsza komórka |
|---|---|---|---|
| sprężyna | 682 / 4107 / 5747 | 38,4% | 2407 |
| rozsuwanie, tylko pozycja | 714 / 3432 / 6095 | 31,8% | 4583 |
| rozsuwanie, pozycja i `LastPosition` | 677 / 3561 / 4942 | 33,5% | 2891 |
| sprężyna + uśrednianie | 74 / 183 / 595 | 0% | 3534 |
| rozsuwanie (pozycja) + uśrednianie | 157 / 402 / 790 | 0% | 338 |
| rozsuwanie (pozycja i `LastPosition`) + uśrednianie | 180 / 372 / 622 | 0% | 1380 |

Wybrane: tylko pozycja (poprawka staje się prędkością, jak w dynamice opartej na pozycjach): najluźniejsze skupiska.

Błąd po drodze: maska przed mnożeniem przez kierunek (0 · NaN na zapasie tablic) dawała NaN.

