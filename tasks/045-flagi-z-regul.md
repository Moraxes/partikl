---
id: 45
title: Flagi z reguł
summary: Flagi wariantów deklaruje moduł reguł, nie forces_defs w Ruście
status: doing
priority: soon
branch: main
part_of: 35
blocked_by:
- 44
created: 2026-11-21T12:39:03
worked: 2026-11-22T00:04:23
edited: 2026-11-22T00:08:15
---
Cel (poziom B): lista shader defs i ich związek z opcjami pochodzi z modułu reguł (a w M3 z modelu), nie z forces_defs. Następny krok: po wymiennym module reguł.


Wynik: opcje deklaruje nagłówek źródła reguł: `//@option nazwa key=K: wybór[=FLAGA], ...` (pierwszy wybór domyślny); parser w options.rs (`RuleSchema`), zasób odświeżany przy wymianie źródła. `SimOptions` to wybory użytkownika po nazwie (bez enum Overlap), `RuleFlags` (flagi dla bieżących wyborów) jest ekstrahowany do świata renderowania i buduje ForcesVariant; forces_defs zna tylko flagi silnika TYPES_AT_MOST_*. HUD i klawisze (N, O w wbudowanych) z deklaracji; zajęte A, B, F, H. CLI: `--option NAME=CHOICE` zamiast --overlap i --no-average-forces (sprawdzane względem deklaracji, błąd = komunikat i wyjście). Bench gpu: warianty z schema.combinations(), nazwy wariantów jak dawniej. Zmiana domyślnych: SimOptions::default() to teraz pierwsze wybory (averaging on, spring); testy wyroczni mają własny Mode. Commit 3ae7552, 63 testy OK. Zostaje: matrix.wesl nadal ładowany asynchronicznie (sporadyczne module not found, 049).
