---
id: 12
title: Profil kroku po SIMD
summary: 'Profil n=9000 na 6808821: update_shape 34–39% kroku, pula wątków ~11% wykorzystania przy małej gęstości'
status: todo
priority: soon
branch: main
created: 2026-11-20T20:29:07
links:
- kind: profile
  target: .artifacts/profiles/sim-quick-6808821-m23.json.gz
  at: 2026-11-20T20:29:44
  note: macierz 23
- kind: profile
  target: .artifacts/profiles/sim-quick-6808821-m6.json.gz
  at: 2026-11-20T20:29:44
  note: macierz 6
---
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
