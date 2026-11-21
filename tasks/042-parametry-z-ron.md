---
id: 42
title: Parametry z ron
summary: RuleParams i macierze ze spec .ron, zapis bufora bez rekompilacji
status: done
priority: soon
branch: main
blocked_by:
- 41
created: 2026-11-21T12:39:03
worked: 2026-11-21T23:41:57
edited: 2026-11-21T23:42:05
---
Cel (poziom A): parametry i macierze z pliku .ron zamiast stałych z config.rs; zmiana = zapis bufora. Naprawić też to, że Buffers.matrix powstaje raz i ignoruje zmiany GpuSim. Następny krok: pola parametrów w ParticleSpec z domyślnymi wartościami z config.rs.

Aktualizacja (2026-11-21, review backlogu): to funkcjonalność, nie czysty refaktor; wydzielone z 035. Obejmuje także 001 (błędy ładowania spec jako Result, powiązane).


Wynik: `ParticleSpec` ma pole `params: RuleParams` (serde, brakujące pola z config.rs; `dt` i `velocity_threshold` pominięte, bo dzielone z silnikiem i renderem). Plik .ron: pełna spec `(interactions, params)` albo stara lista wierszy. `ParticleSpec` jest ekstrahowany do świata renderowania, `buffers::upload_spec` zapisuje macierz i RuleParams przy zmianie zasobu (bez rekompilacji); `GpuSim.matrix` usunięte. Walidacja: kernel_middle + kernel_width ≤ INTERACTION_RADIUS (geometria siatki nadal ze stałej; zasięg z ron wymagałby Geometry z params). Wyrocznia f64 czyta params ze spec; test `spec_changes_at_runtime`. Commit b6eb54c. Obejmuje też 001: `loading` zwraca `Result<_, LoadError>`, testy w loading/tests.rs. 51 testów OK.
