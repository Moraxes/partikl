---
id: 58
title: Profil GPU w Xcode
summary: 'Liczniki GPU (Instruments/Metal): czy pętla sił jest ograniczona obliczeniami czy opóźnieniami'
status: todo
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
---
Cel: potwierdzić lub obalić hipotezę, że pętla sił nie jest ograniczona pamięcią, zanim zainwestujemy w M2 3.1 (SoA/kompresja) i 3.3 (Morton). Wymaga pełnego Xcode (na maszynie są tylko Command Line Tools; xctrace/Instruments niedostępne): Metal System Trace i GPU counters, ewentualnie przechwycenie klatki. Bez Xcode zostaje HUD Metal (MTL_HUD_ENABLED=1), który pokazuje tylko czas klatki. Następny krok: instalacja Xcode przez użytkownika.
