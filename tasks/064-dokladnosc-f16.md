---
id: 64
title: Dokładność f16
summary: Testy i kryteria dokładności ścieżki f16, ocena wyglądu, decyzja o domyślnym
status: todo
priority: later
branch: compute-shader-1
part_of: 62
blocked_by:
- 63
created: 2026-11-21T19:19:16
edited: 2026-11-21T19:25:55
---
Cel: ustalić, czy f16 jest akceptowalne fizycznie. Testy: (1) krok vs wyrocznia f64: max błąd pozycji po 1 kroku (dziś tolerancja 1e-3; dla f16 oczekiwane ok. 1e-2, ustalić z danych: błąd względny sił ok. 2^-10); (2) dryf po 100–500 krokach w trybie deterministycznym: brak NaN, cząstki w świecie, zachowana liczba i id, porównanie statystyk (średnia energia kinetyczna, rozkład prędkości, liczba sąsiadów) f16 vs f32 na tym samym stanie startowym; (3) galeria wyglądu (bench render --gallery) f16 vs f32 po kilkuset krokach, na oko. Bramka: statystyki w granicach szumu między przebiegami f32 (atomowa niedeterministyczność), brak widocznej różnicy; decyzja o domyślnym z użytkownikiem. Następny krok: po wariancie PAIR_F16.

Decyzja o domyślnym: patrz polityka w 062 (akceptacja wizualna użytkownika w ruchu, chyba że > 25% szybciej). Do akceptacji użytkownik przełącza P w działającej aplikacji (f32/f16 na tym samym stanie).
