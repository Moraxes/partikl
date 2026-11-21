---
id: 59
title: Zegar kroków przy przeciążeniu
summary: Gdy krok GPU > budżet czasu, symulacja zwalnia zamiast wydłużać klatkę do 140 ms
status: done
priority: soon
branch: compute-shader-1
part_of: 54
relates:
- 55
created: 2026-11-21T16:44:13
worked: 2026-11-21T17:55:44
edited: 2026-11-21T17:49:23
links:
- kind: commit
  target: a0e5260
  at: 2026-11-21T17:49:23
  note: 'feat(gpu): slow the simulation down instead of stretching frames'
- kind: commit
  target: '3724203'
  at: 2026-11-21T17:55:44
  note: 'feat(bench): headless application loop with the real fixed-step clock'
---
Cel: przy 1M cząstek (świat 1800×1169, ok. 4700 kandydatów, próbkowanie do 512) krok kosztuje ok. 11 ms, a stały krok to 10 ms, więc fixed timestep goni zaległość (do MAX_STEPS_PER_FRAME = 16 kroków w klatce): klatka ok. 140 ms (7 fps), UI i obraz zacinają się. Lepsza polityka: budżet czasu na klatkę (kroki do limitu pomiarem czasu GPU/klatki), symulacja zwalnia (mniej kroków na sekundę), obraz płynny; wskaźnik 'x realtime' w HUD. Wynik: src/gpu/step_budget.rs. Limit kroków na klatkę (StepBudget::limit), takty ponad limit przepadają; regulator AIMD po czasie klatki (TARGET_FRAME 20 ms: klatka dłuższa i więcej niż 1 krok → limit ×0,7; krótka klatka z przepadłymi taktami → +1; twardy sufit MAX_STEPS_PER_FRAME 16). Prędkość symulacji (średnia kroków/taktów) jako diagnostyka partikl/sim_speed i linia 'sim speed N%' w HUD poniżej 97%. Aplikacja, release, bez vsync (klatka / prędkość): 1M 174 ms → ok. 17 ms (58 fps) przy ok. 70%; 800k ok. 14,5 ms przy ok. 91%; 500k ok. 6 ms przy 100% (bez zmian). Czas klatki przy 1M oscyluje 15–25 ms (piła regulatora AIMD). Pomiar czasu klatki zamiast GPU timestamp: render jest potokowy, więc główna pętla czeka na wolniejszy koniec. Zostaje: 1M nadal ok. 70% czasu rzeczywistego, bo krok GPU ok. 11 ms przy gęstości 4700 kandydatów; realne przyspieszenie wymaga optymalizacji przebiegu sił (zadanie 058 i M2).
