---
id: 12
title: Profil kroku po SIMD
summary: 'Krok n=9000: ×0,64 / ×0,61 / ×0,33 (macierze 23 / 6 / 4) po zadaniach 013–019'
status: done
priority: soon
branch: main
relates:
- 10
created: 2026-11-20T20:29:07
worked: 2026-11-20T22:44:36
edited: 2026-11-20T22:44:36
links:
- kind: profile
  target: .artifacts/profiles/sim-quick-6808821-m23.json.gz
  at: 2026-11-20T20:29:44
  note: macierz 23
- kind: profile
  target: .artifacts/profiles/sim-quick-6808821-m6.json.gz
  at: 2026-11-20T20:29:44
  note: macierz 6
- kind: profile
  target: .artifacts/profiles/sim-quick-6808821-m4.json.gz
  at: 2026-11-20T20:29:44
  note: macierz 4
- kind: report
  target: .artifacts/reports/perf-report.md
  at: 2026-11-20T22:44:36
  note: sekcja Tryb szybki
---
Wynik: krok przy n=9000 (tryb `--quick`, iloczyn etapów) ×0,64 / ×0,61 / ×0,33 dla macierzy 23 / 6 / 4: z 392 / 478 / 1329 µs do ok. 223 / 290 / 426 µs. Bez próbkowania sąsiadów (zadanie 019, zmienia wygląd gęstych skupisk) macierz 4 ×0,70 (ok. 890 µs). Etapy i tabele w raporcie, sekcja „Tryb szybki”.

| zadanie | wynik |
|---|---|
| 013 obrót bez trygonometrii | krok ×0,76 / ×0,72 / ×0,87 |
| 014 siatka przez sortowanie | krok ×0,97 / ×0,92 / ×0,89 (inne stany) |
| 015 siły w kolejności siatki | krok ×0,86 / ×1,03 / ×1,00 |
| 016 podział po równo, komórki R/2 | krok ×0,93 / ×1,02 / ×1,01, potem ×1,07 / ×0,91 / ×0,85 |
| 017 pary liczone raz (z poprawką determinizmu) | `compute_forces` na kandydata ×1,00 / ×0,84 / ×0,75 |
| 019 próbkowanie powyżej 512 kandydatów | krok ×1,01 / ×0,97 / ×0,47; wygląd do oceny |
| 021 usypianie | bez zmian: brak spokojnych cząstek |

Przy kilku etapach trajektorie się zmieniły (kolejność sumowania), więc iloczyn dla macierzy 6 i 4 jest przybliżony.

## Profil z punktu wyjścia
Zadanie zbiorcze dla optymalizacji z profilu kroku przy n=9000 (tryb `--quick`, commit 6808821, samply 4 kHz, osobno macierze 23, 6 i 4). Następny krok: zacząć od zadania z kształtem bez trygonometrii (najtańsze, największy zysk przy typowych macierzach), potem przebudowa siatki i kolejność sił.

## Profil (czas ścienny wątku głównego)
| | 23 (wolny wzrost) | 6 (typowy) | 4 (szybki) |
|---|---|---|---|
| krok | 408 µs | 499 µs | 1324 µs |
| `compute_forces` | 43% | 51% | 78% |
| `update_shape` | 39% | 34% | 14% |
| `wrap_around` | 13% | 11% | 6% |
| `integrate` + `compute_friction` | 5% | 5% | 2% |

- `update_shape`: ok. 170 µs na krok niezależnie od macierzy (19 ns na cząstkę); ok. 2/3 to `__sincosf_stret` z `Quat::from_rotation_z`, reszta `log2f` i `angle_to`. Tylko wizualne, jeden wątek.
- `compute_forces`, macierz 23: wątki puli zajęte w ok. 4% próbek, łączna praca jądra ok. 190 µs CPU w oknie ok. 155 µs na 11 wątkach (wykorzystanie ok. 11%); obsługa zadań (`mutexwait`, `cvwait`, tworzenie i zwalnianie zadań) to ok. 13% kroku na wątku głównym. Macierz 4: wykorzystanie ok. 40%, wątek główny czeka 12% kroku na najwolniejsze wątki.
- `wrap_around`: 50–80 µs, `move_entity` po kolei ze sprawdzeniem slotu; `ForceData::fill` seryjnie ok. 6% kroku przy macierzy 23.
- Macierz 4: 1220 kandydatów w 3×3 komórkach po 40, czyli odstęp ok. 3,4 przy promieniu odpychania 10; cząstki w skupiskach nachodzą na siebie.
- Przybliżone `1/√x` odpada: wolniejsze na M5 (zadanie 004).
- Bench nie mierzy renderowania; zmiany `Transform` co krok kosztują też w ekstrakcji do renderera.

## Profil: jak czytać
`samply load .artifacts/profiles/sim-quick-6808821-m<macierz>.json.gz`. Nagrane: `samply record -s --unstable-presymbolicate -r 4000 -- <bench sim> --n 9000 --offsets 4096 --matrices 1 --seed-base <macierz> --measured 100`. Symbole libsystem_m są w pliku tylko jako przesunięcia (np. +0x338c to `__sincosf_stret`, adres z `dladdr`).
