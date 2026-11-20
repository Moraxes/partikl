---
id: 11
title: Zapis stanu benchmarku
summary: Stan symulacji po offsecie z cache zamiast symulowania 4096 kroków; pomiar przed/po na tym samym stanie
status: doing
priority: later
branch: main
created: 2026-11-20T19:56:05
worked: 2026-11-21T00:43:45
edited: 2026-11-21T00:45:19
links:
- kind: commit
  target: c8e2412
  at: 2026-11-21T00:43:45
  note: 'feat(bench): measure quick runs from saved simulation states'
---
Wynik: `sim --state` (stany w `.cache/sim-states`, `--regenerate`, `--prewarm 500`) i `ab.sh --state` (stany generowane kodem „przed”, obie strony mierzą ten sam stan). Przebieg `--quick` ok. 0,8 s zamiast ok. 4,2 s.

## Zapisywany stan
Cząstki w kolejności ECS (pozycja, poprzednia pozycja, typ) i `ForceData::carried` (praca z poprzedniego kroku i numer kroku). Bez stanu `ForceData` pierwszy krok po wczytaniu dzieliłby pracę inaczej i sumował bufory w innej kolejności, więc trajektoria rozjechałaby się w ostatnich bitach. `Crowding` niezapisywany (wariant domyślny go nie używa).

## Porównanie z samym `--quick` (ta sama rewizja, 3 rundy na przemian)
| | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| sekwencje kandydatów | identyczne | identyczne | identyczne |
| krok, stan bez rozgrzewki | ×1,03 | ×1,12 | ×1,09 |
| krok, stan + 500 kroków rozgrzewki | ×0,93 | ×1,01 | ×0,99 |
| krok, stan + 2000 kroków rozgrzewki | ×0,94 | ×0,99 | ×1,00 |
| krok, zwykły vs zwykły (szum) | ×1,07 (rundy 0,96–1,31) | ×0,99 | ×1,00 |

Bez rozgrzewki pomiar zaczyna się kilka kroków po starcie procesu (procesor i pula „nierozpędzone”). Rozgrzewka liczy kroki od wczytanego stanu i wraca do niego, więc mierzone kroki są te same. Macierz 23 (najmniejsze zadania puli) ma rozrzut między procesami rzędu ±10% niezależnie od trybu.

`ab.sh 3 --state` na tym samym kodzie: ×0,95 / ×1,03 / ×1,00, te same stany.

## Ograniczenia
- Stan z rewizji „przed”: nowy kod mierzony na stanie starej dynamiki; zmian wyglądu (np. gęstsze skupiska) tak nie widać. Do tego zwykły `--quick` albo pełny bench.
- Stany w `.cache/sim-states` nie są unieważniane same; po zmianie fizyki `--regenerate` (`ab.sh` regeneruje zawsze).

