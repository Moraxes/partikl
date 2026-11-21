---
id: 42
title: Parametry z ron
summary: RuleParams i macierze ze spec .ron, zapis bufora bez rekompilacji
status: todo
priority: soon
branch: compute-shader-1
blocked_by:
- 41
created: 2026-11-21T12:39:03
edited: 2026-11-21T22:43:15
---
Cel (poziom A): parametry i macierze z pliku .ron zamiast stałych z config.rs; zmiana = zapis bufora. Naprawić też to, że Buffers.matrix powstaje raz i ignoruje zmiany GpuSim. Następny krok: pola parametrów w ParticleSpec z domyślnymi wartościami z config.rs.

Aktualizacja (2026-11-21, review backlogu): to funkcjonalność, nie czysty refaktor; wydzielone z 035. Obejmuje także 001 (błędy ładowania spec jako Result, powiązane).
