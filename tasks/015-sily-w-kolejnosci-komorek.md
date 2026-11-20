---
id: 15
title: Siły w kolejności komórek
summary: 'Siły w kolejności siatki, zakresy o równej pracy: compute_forces ×0,78 przy małej gęstości, bez zmian przy dużej'
status: done
priority: soon
branch: main
part_of: 12
blocked_by:
- 14
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:04:42
edited: 2026-11-20T21:04:42
links:
- kind: commit
  target: 32c0e8c
  at: 2026-11-20T21:04:16
  note: 'perf: compute forces in grid order over particle ranges of balanced work'
---
Wynik: siły w kolejności siatki, zakresami cząstek o zbliżonej pracy (`32c0e8c`): `compute_forces` ×0,78 przy małej gęstości (macierz 23), bez zmian przy 6 i 4; krok ×0,86 / ×1,03 / ×1,00. Lokalność danych przy gęstych macierzach nic nie dała; zysk przy 23 to mniej zadań puli. Rayon odrzucony.

## Wyniki (`--quick`, 3 rundy na przemian, te same stany, `compute_forces` po/przed [95% CI])
| wariant | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| zakresy komórek, 4 na wątek | 0,82 [0,80; 0,83] | 0,96 [0,94; 0,97] | 2,31 [2,25; 2,42] |
| zakresy cząstek, 4 na wątek | 0,89 [0,88; 0,90] | 0,97 [0,95; 0,99] | 1,05 [1,02; 1,08] |
| zakresy cząstek, 16 na wątek | 1,13 [1,10; 1,15] | 1,09 [1,07; 1,10] | 1,01 [0,98; 1,04] |
| ok. 50 tys. par na zakres, 1–16 na wątek (`32c0e8c`) | 0,78 [0,77; 0,81] | 1,00 [0,98; 1,01] | 1,00 [0,97; 1,03] |
| to samo z rayon zamiast puli Bevy | 1,21 [1,17; 1,26] | 1,14 [1,13; 1,16] | 1,06 [1,04; 1,09] |

- Zakresy komórek: komórka w gęstym skupisku (setki cząstek po ponad 1000 kandydatów) to jedno zadanie, stąd ×2,3 przy macierzy 4.
- Liczba zadań: przy małej gęstości każde zadanie puli kosztuje więcej niż jego praca; przy dużej trzeba kilkunastu na wątek, żeby wyrównać rdzenie wydajne i energooszczędne. Stąd liczba zakresów z łącznej pracy.
- Rayon (osobna pula obok bezczynnej puli Bevy): wolniejszy w każdej macierzy.

Dane: etap w `.artifacts/perf-stages.json` (`quick`), raport `.artifacts/reports/perf-report.md`; przebiegi odrzuconych wariantów w `.artifacts/bench/` z 2026-11-20 20:54–21:04.

## Pierwotny plan
Cel: podnieść wykorzystanie wątków w `compute_forces` (ok. 11% przy macierzy 23, ok. 40% przy 4) i lokalność danych. Następny krok: iterować po cząstkach w kolejności komórek z tablic z sortowania (kolejne cząstki mają tych samych sąsiadów w L1), w zakresach ciągłych zamiast `par_iter_mut` w kolejności ECS; przyspieszenia do tablicy, potem przepisać do `Acceleration`.

## Do sprawdzenia
- Liczba i rozmiar zadań: mniej, większych przy małej gęstości; podział według liczby kandydatów (suma prefiksowa) zamiast według liczby cząstek, żeby wyrównać skupiska.
- rayon zamiast wykonawcy zadań Bevy (`mutexwait`, `cvwait`, tworzenie zadań to ok. 13% kroku przy macierzy 23).
- Rdzenie wydajne i energooszczędne: przy macierzy 4 wątek główny czeka 12% kroku na najwolniejsze wątki.
