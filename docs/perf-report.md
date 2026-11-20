# Raport wydajności

Etapy optymalizacji symulacji i indeksu przestrzennego, mierzone benchmarkami `sim` i `grid`
(`cargo bench`). Liczby to średnia geometryczna stosunku czasów nowy/stary na tych samych
`(n, macierz, offset)`; poniżej 1 znaczy szybciej. Plik generuje
`node tools/bench-report/report.js` z `.data/perf-stages.json`; nie edytować ręcznie.

## Względem poprzedniego etapu

| Etap | Commit | krok | compute_forces | compute_friction | integrate | wrap_around | update_shape | grid: neighbours | grid: build | grid: move |
|---|---|---|---|---|---|---|---|---|---|---|
| punkt wyjścia | `8f682a0` | — | — | — | — | — | — | — | — | — |
| płaska siatka | `7eaa030` | 0,58 | 0,52 | 1,04 | 1,09 | 0,61 | 1,04 | 0,29 | 0,40 | 0,79 |
| siły z danych w siatce | `71fd918` | 0,81 | 0,74 | 1,05 | 0,92 | 1,94 | 0,94 | 1,16 | 1,20 | 1,18 |

## Względem punktu wyjścia

Iloczyn wyników kolejnych etapów.

| Etap | Commit | krok | compute_forces | compute_friction | integrate | wrap_around | update_shape | grid: neighbours | grid: build | grid: move |
|---|---|---|---|---|---|---|---|---|---|---|
| punkt wyjścia | `8f682a0` | 1,00 | 1,00 | 1,00 | 1,00 | 1,00 | 1,00 | 1,00 | 1,00 | 1,00 |
| płaska siatka | `7eaa030` | 0,58 | 0,52 | 1,04 | 1,09 | 0,61 | 1,04 | 0,29 | 0,40 | 0,79 |
| siły z danych w siatce | `71fd918` | 0,48 | 0,39 | 1,09 | 0,99 | 1,17 | 0,97 | 0,33 | 0,48 | 0,93 |

## Czas kroku symulacji

Mediana po macierzach z median czasu kroku w µs, z przebiegów każdego etapu (różne dni, więc
zawierają dryf warunków; zmiany etapów mierzą tabele wyżej).

| n | offset | punkt wyjścia | płaska siatka | siły z danych w siatce |
|---|---|---|---|---|
| 1000 | 256 | 152 | 69 | 63 |
| 1000 | 1024 | 152 | 72 | 65 |
| 1000 | 4096 | 158 | 81 | 77 |
| 3000 | 256 | 503 | 247 | 212 |
| 3000 | 1024 | 517 | 273 | 237 |
| 3000 | 4096 | 613 | 413 | 351 |
| 9000 | 256 | 1877 | 1154 | 927 |
| 9000 | 1024 | 2249 | 1603 | 1189 |
| 9000 | 4096 | 3927 | 3175 | 2433 |

## Etapy

### 0. punkt wyjścia (`8f682a0`)

Siatka na `HashMap`, zapytanie o sąsiadów alokuje listę zawiniętych komórek.

- sim: `sim-2026-11-20-03-05-44.jsonl`, rewizja `f43058b*`, 288 rekordów
- grid: `grid-2026-11-20-03-42-18.jsonl`, rewizja `8f682a0`, 864 rekordów

### 1. płaska siatka (`7eaa030`)

Komórki w płaskim `Vec`, zawinięte komórki w tablicy o stałej długości, ta sama kolejność przeglądania sąsiadów.

- sim: `sim-2026-11-20-04-02-49.jsonl`, rewizja `b1e67ad*`, 288 rekordów
- grid: `grid-2026-11-20-04-02-44.jsonl`, rewizja `b1e67ad*`, 864 rekordów

Trajektorie identyczne jak przed zmianą (288/288 punktów), porównanie na tych samych stanach.

| Metryka | ogółem [95% CI] | n=1000 | n=3000 | n=9000 |
|---|---|---|---|---|
| krok | 0,58 [0,57; 0,60] | 0,44 / 0,47 / 0,54 | 0,53 / 0,55 / 0,65 | 0,64 / 0,77 / 0,76 |
| compute_forces | 0,52 [0,51; 0,54] | 0,37 / 0,40 / 0,47 | 0,45 / 0,49 / 0,60 | 0,60 / 0,74 / 0,74 |
| compute_friction | 1,04 [1,02; 1,05] | 0,95 / 0,97 / 0,98 | 1,15 / 1,15 / 1,07 | 1,03 / 1,04 / 1,02 |
| integrate | 1,09 [1,07; 1,10] | 1,06 / 1,08 / 1,06 | 1,19 / 1,15 / 1,09 | 1,03 / 1,10 / 1,03 |
| wrap_around | 0,61 [0,60; 0,61] | 0,55 / 0,55 / 0,57 | 0,64 / 0,64 / 0,63 | 0,60 / 0,64 / 0,65 |
| update_shape | 1,04 [1,03; 1,05] | 1,02 / 1,03 / 1,02 | 1,11 / 1,02 / 1,05 | 1,04 / 1,05 / 1,03 |
| grid: neighbours | 0,29 [0,28; 0,29] | 0,24 / 0,25 / 0,28 | 0,27 / 0,27 / 0,31 | 0,31 / 0,32 / 0,35 |
| grid: build | 0,40 [0,39; 0,41] | 0,38 / 0,40 / 0,42 | 0,40 / 0,41 / 0,40 | 0,41 / 0,40 / 0,37 |
| grid: move | 0,79 [0,77; 0,81] | 0,86 / 0,92 / 0,85 | 0,75 / 0,77 / 0,77 | 0,70 / 0,72 / 0,77 |

Kolumny n: offsety 256 / 1024 / 4096.

### 2. siły z danych w siatce (`71fd918`)

Siatka trzyma pozycję i typ każdej cząstki oraz jej miejsce w komórce (pozycja aktualizowana co krok w `wrap_around` bez szukania w komórce), `compute_forces` nie pyta ECS o sąsiadów i pisze przyspieszenie bezpośrednio, bez kolejki `Parallel`.

- przed, sim: `sim-2026-11-20-13-11-36.jsonl`, rewizja `2507bff`, 288 rekordów
- przed, grid: `grid-2026-11-20-13-22-46.jsonl`, rewizja `2507bff`, 864 rekordów
- po, sim: `sim-2026-11-20-13-22-55.jsonl`, rewizja `2507bff*`, 288 rekordów
- po, grid: `grid-2026-11-20-13-31-14.jsonl`, rewizja `2507bff*`, 864 rekordów

Porównanie z pomiarem „przed” zrobionym tuż przed pomiarem etapu.

Trajektorie identyczne jak przed zmianą (288/288 punktów), porównanie na tych samych stanach.

| Metryka | ogółem [95% CI] | n=1000 | n=3000 | n=9000 |
|---|---|---|---|---|
| krok | 0,81 [0,80; 0,83] | 0,88 / 0,90 / 0,89 | 0,83 / 0,83 / 0,81 | 0,78 / 0,67 / 0,76 |
| compute_forces | 0,74 [0,73; 0,76] | 0,78 / 0,80 / 0,81 | 0,74 / 0,74 / 0,75 | 0,72 / 0,63 / 0,72 |
| compute_friction | 1,05 [1,03; 1,06] | 1,07 / 1,06 / 1,05 | 1,10 / 1,12 / 1,08 | 1,01 / 0,95 / 1,00 |
| integrate | 0,92 [0,90; 0,93] | 0,88 / 0,83 / 0,82 | 0,99 / 1,00 / 0,96 | 0,97 / 0,88 / 0,94 |
| wrap_around | 1,94 [1,85; 2,03] | 2,48 / 2,47 / 2,29 | 2,18 / 2,06 / 1,72 | 1,85 / 1,50 / 1,26 |
| update_shape | 0,94 [0,92; 0,95] | 0,95 / 0,93 / 0,91 | 0,93 / 0,93 / 0,92 | 0,97 / 0,92 / 0,96 |
| grid: neighbours | 1,16 [1,14; 1,17] | 1,09 / 1,04 / 1,06 | 1,21 / 1,21 / 1,20 | 1,17 / 1,19 / 1,26 |
| grid: build | 1,20 [1,18; 1,21] | 1,12 / 1,15 / 1,18 | 1,16 / 1,19 / 1,22 | 1,22 / 1,25 / 1,28 |
| grid: move | 1,18 [1,13; 1,24] | 1,30 / 1,25 / 1,23 | 1,36 / 1,32 / 1,04 | 1,41 / 1,12 / 0,78 |

Kolumny n: offsety 256 / 1024 / 4096.
